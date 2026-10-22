use rusqlite::{Connection, OpenFlags, Result};
use std::path::Path;

fn main() -> Result<()> {
    let source_path = "sqlcipher-failure-source.db";
    let target_path = "sqlcipher-failure-target.db";

    // Не перезаписываем файлы предыдущих запусков.
    for path in [
        source_path,
        "sqlcipher-failure-source.db-wal",
        "sqlcipher-failure-source.db-shm",
        "sqlcipher-failure-source.db-journal",
        target_path,
        "sqlcipher-failure-target.db-wal",
        "sqlcipher-failure-target.db-shm",
        "sqlcipher-failure-target.db-journal",
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
        VALUES ('entry-1', 'Исходная запись');
        ",
    )?;

    drop(source);

    println!("1. Искусственная исходная база создана.");

    // 2. Открываем исходную базу только для чтения.
    let original = Connection::open_with_flags(
        source_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;

    // 3. Намеренно вызываем ошибку SQLCipher:
    // создаём целевой файл, затем пытаемся экспортировать
    // данные в несуществующую схему.
    let target = Connection::open(target_path)?;

    target.execute_batch(
        "
        PRAGMA key = 'laboratory-password';
        CREATE TABLE placeholder (id INTEGER);
        ",
    )?;

    drop(target);

    println!("2. Искусственная целевая база создана.");

    // ATTACH создаёт подключённую базу с указанным ключом.
    // Экспорт намеренно направляем в неправильную схему.
    original.execute_batch(
        "
        ATTACH DATABASE 'sqlcipher-failure-target.db'
        AS encrypted
        KEY 'laboratory-password';
        ",
    )?;

    let export_result = original.execute_batch(
        "SELECT sqlcipher_export('nonexistent_schema');",
    );

    assert!(
        export_result.is_err(),
        "Ожидалась ошибка SQLCipher-экспорта"
    );

    println!("3. SQLCipher-экспорт завершился ожидаемой ошибкой.");

    // 4. Проверяем, что исходная база сохранила данные.
    let content: String = original.query_row(
        "SELECT content FROM main.entries WHERE id = 'entry-1'",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(content, "Исходная запись");

    let count: i64 = original.query_row(
        "SELECT COUNT(*) FROM main.entries",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(count, 1);

    println!("4. Исходная база сохранила запись.");

    // 5. Проверяем, что оба файла существуют отдельно.
    assert!(Path::new(source_path).exists());
    assert!(Path::new(target_path).exists());

    println!("5. Исходный и целевой файлы остались раздельными.");
    println!("Тест ошибки SQLCipher-экспорта успешно пройден!");

    Ok(())
}