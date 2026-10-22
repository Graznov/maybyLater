
use rusqlite::{params, Connection, Result};
use std::path::Path;

fn main() -> Result<()> {
    let source_path = "wal-source.db";
    let encrypted_path = "wal-encrypted.db";
    let password = "laboratory-wal-password";

    // Не перезаписываем результаты предыдущих экспериментов.
    for path in [
        source_path,
        "wal-source.db-wal",
        "wal-source.db-shm",
        encrypted_path,
        "wal-encrypted.db-wal",
        "wal-encrypted.db-shm",
    ] {
        if Path::new(path).exists() {
            println!("Файл {path} уже существует. Ничего не изменено.");
            return Ok(());
        }
    }

    // 1. Создаём обычную SQLite-базу в режиме WAL.
    let source = Connection::open(source_path)?;

    source.execute_batch(
        "
        PRAGMA journal_mode = WAL;

        CREATE TABLE entries (
            id TEXT PRIMARY KEY NOT NULL,
            content TEXT NOT NULL
        );

        INSERT INTO entries (id, content)
        VALUES ('entry-1', 'Первая тестовая запись');
        ",
    )?;

    let journal_mode: String =
        source.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;

    assert_eq!(journal_mode.to_lowercase(), "wal");

    println!("1. Исходная база работает в режиме WAL.");

    // 2. Добавляем запись, пока соединение остаётся открытым.
    source.execute(
        "INSERT INTO entries (id, content) VALUES (?1, ?2)",
        params!["entry-2", "Запись, добавленная в режиме WAL"],
    )?;

    // Удерживаем читающую транзакцию на втором соединении,
    // чтобы checkpoint не мог полностью очистить WAL.
    let reader = Connection::open(source_path)?;
    reader.execute_batch("BEGIN;")?;

    let count_before: i64 =
        reader.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))?;

    assert_eq!(count_before, 2);

    source.execute(
        "INSERT INTO entries (id, content) VALUES (?1, ?2)",
        params!["entry-3", "Последняя тестовая запись"],
    )?;

    assert!(Path::new("wal-source.db-wal").exists());

    println!("2. Созданы записи при активном WAL-файле.");

    // 3. Экспортируем базу через SQLCipher.
    // Не копируем основной .db-файл вручную.
    source.execute(
        "ATTACH DATABASE ?1 AS encrypted KEY ?2",
        params![encrypted_path, password],
    )?;

    source.query_row(
        "SELECT sqlcipher_export('encrypted')",
        [],
        |_row| Ok(()),
    )?;

    source.execute_batch("DETACH DATABASE encrypted;")?;

    println!("3. Экспорт в SQLCipher завершён.");

    // 4. Проверяем зашифрованную копию.
    let encrypted = Connection::open(encrypted_path)?;
    encrypted.pragma_update(None, "key", password)?;

    let encrypted_count: i64 =
        encrypted.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))?;

    assert_eq!(encrypted_count, 3);

    let latest_content: String = encrypted.query_row(
        "SELECT content FROM entries WHERE id = 'entry-3'",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(latest_content, "Последняя тестовая запись");

    println!("4. Все три записи присутствуют в зашифрованной копии.");

    // 5. Исходная база остаётся доступной.
    let source_count: i64 =
        source.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))?;

    assert_eq!(source_count, 3);

    println!("5. Исходная SQLite-база сохранила все записи.");

    // 6. Закрываем соединения штатно.
    drop(encrypted);
    drop(reader);
    drop(source);

    println!("6. Соединения закрыты.");
    println!("Проверка переноса данных при активном WAL завершена успешно!");

    Ok(())
}
