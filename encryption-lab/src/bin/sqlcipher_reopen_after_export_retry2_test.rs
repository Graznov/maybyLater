use rusqlite::{params, Connection, OpenFlags, Result};
use std::path::Path;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let source_path = "reopen-export-retry2-source.db";
    let encrypted_path = "reopen-export-retry2-encrypted.db";
    let password = "laboratory-password-18";

    // Не перезаписываем файлы предыдущих запусков.
    for path in [
        source_path,
        "reopen-export-retry2-source.db-wal",
        "reopen-export-retry2-source.db-shm",
        "reopen-export-retry2-source.db-journal",
        encrypted_path,
        "reopen-export-retry2-encrypted.db-wal",
        "reopen-export-retry2-encrypted.db-shm",
        "reopen-export-retry2-encrypted.db-journal",
    ] {
        if Path::new(path).exists() {
            println!("Файл {path} уже существует. Ничего не изменено.");
            return Ok(());
        }
    }

    // 1. Создаём искусственную исходную базу SQLite.
    let source = Connection::open(source_path)?;

    source.execute_batch(
        "
        CREATE TABLE entries (
            id TEXT PRIMARY KEY NOT NULL,
            content TEXT NOT NULL
        );

        INSERT INTO entries (id, content)
        VALUES ('entry-18', 'Секретная лабораторная запись');
        ",
    )?;

    drop(source);

    println!("1. Искусственная исходная база создана.");

    // 2. Открываем исходную базу тем же способом,
    // что и в успешном migration_test.rs.
    let original = Connection::open(source_path)?;

    // 3. Подключаем целевую базу через параметры.
    original.execute(
        "ATTACH DATABASE ?1 AS encrypted KEY ?2",
        params![encrypted_path, password],
    )?;

    println!("2. Целевая база успешно подключена.");

    // 4. Выполняем настоящий SQLCipher-экспорт.
    original.query_row(
        "SELECT sqlcipher_export('encrypted')",
        [],
        |_row| Ok(()),
    )?;

    original.execute_batch("DETACH DATABASE encrypted;")?;

    drop(original);

    println!("3. SQLCipher-экспорт успешно завершён.");

    // 5. Проверяем заголовок зашифрованного файла.
    let header = std::fs::read(encrypted_path)?;

    assert!(header.len() >= 16);

    assert_ne!(
        &header[..16],
        b"SQLite format 3\0",
        "Целевая база не должна иметь открытый заголовок SQLite"
    );

    println!("4. Заголовок зашифрованной базы проверен.");

    // 6. Повторно открываем файл с правильным паролем.
    let correct = Connection::open_with_flags(
        encrypted_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;

    correct.pragma_update(None, "key", password)?;

    let content: String = correct.query_row(
        "SELECT content FROM entries WHERE id = 'entry-18'",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(content, "Секретная лабораторная запись");

    println!("5. База повторно открыта с правильным паролем.");

    drop(correct);

    // 7. Проверяем неправильный пароль.
    let wrong = Connection::open_with_flags(
        encrypted_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;

    wrong.pragma_update(None, "key", "wrong-password")?;

    let wrong_result: Result<i64> = wrong.query_row(
        "SELECT COUNT(*) FROM entries",
        [],
        |row| row.get(0),
    );

    assert!(
        wrong_result.is_err(),
        "Неправильный пароль не должен открывать данные"
    );

    println!("6. Неправильный пароль не открыл данные.");

    drop(wrong);

    // 8. Проверяем исходную базу после экспорта.
    let original_check = Connection::open_with_flags(
        source_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;

    let original_content: String = original_check.query_row(
        "SELECT content FROM entries WHERE id = 'entry-18'",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(
        original_content,
        "Секретная лабораторная запись"
    );

    println!("7. Исходная база сохранила запись.");
    println!("Повторный тест SQLCipher успешно пройден!");

    Ok(())
}