
use rusqlite::{params, Connection, Result};
use std::path::Path;

fn read_content(conn: &Connection) -> Result<String> {
    conn.query_row(
        "SELECT content FROM entries WHERE id = 'entry-1'",
        [],
        |row| row.get(0),
    )
}

fn main() -> Result<()> {
    let source_path = "interrupted-source.db";
    let encrypted_path = "interrupted-encrypted.db";
    let password = "laboratory-interruption-password";

    // Не перезаписываем существующие лабораторные файлы.
    if Path::new(source_path).exists() || Path::new(encrypted_path).exists() {
        println!("Тестовые файлы уже существуют. Ничего не изменено.");
        return Ok(());
    }

    // 1. Создаём исходную SQLite-базу с тестовой записью.
    let source = Connection::open(source_path)?;

    source.execute_batch(
        "
        CREATE TABLE entries (
            id TEXT PRIMARY KEY NOT NULL,
            content TEXT NOT NULL
        );

        INSERT INTO entries (id, content)
        VALUES ('entry-1', 'Важная тестовая запись');
        ",
    )?;

    let original_content = read_content(&source)?;
    assert_eq!(original_content, "Важная тестовая запись");

    println!("1. Исходная SQLite-база создана.");

    drop(source);

    // 2. Начинаем подготовку зашифрованной копии.
    let source = Connection::open(source_path)?;

    source.execute(
        "ATTACH DATABASE ?1 AS encrypted KEY ?2",
        params![encrypted_path, password],
    )?;

    println!("2. Зашифрованная база подключена.");

    // 3. Имитируем сбой ДО вызова sqlcipher_export().
    //
    // Мы намеренно не копируем данные и не заменяем исходную базу.
    // Это безопасная имитация раннего прерывания,
    // а не настоящего аварийного завершения процесса.
    println!("3. Смоделировано прерывание до переноса данных.");

    // Закрываем подключение штатно.
    source.execute_batch("DETACH DATABASE encrypted;")?;
    drop(source);

    // 4. Проверяем исходную базу после прерывания.
    let original = Connection::open(source_path)?;

    let content_after_interruption = read_content(&original)?;
    assert_eq!(content_after_interruption, original_content);

    println!("4. Исходная запись сохранилась без изменений.");

    // 5. Проверяем целостность исходной SQLite-базы.
    let integrity: String = original.query_row(
        "PRAGMA integrity_check",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(integrity, "ok");

    println!("5. Проверка целостности исходной базы успешна.");

    drop(original);

    // 6. Убеждаемся, что неполная зашифрованная база
    // не содержит скопированной таблицы дневника.
    let encrypted = Connection::open(encrypted_path)?;
    encrypted.pragma_update(None, "key", password)?;

    let copied_tables: i64 = encrypted.query_row(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'table' AND name = 'entries'",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(copied_tables, 0);

    println!("6. Неполная копия не содержит записей дневника.");

    println!("Тест раннего прерывания завершён успешно!");

    Ok(())
}
