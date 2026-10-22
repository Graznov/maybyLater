
use rusqlite::{params, Connection, Result};
use std::path::Path;

type MigrationRow = (i64, String, String, bool, Vec<u8>, i64);

fn read_history(conn: &Connection) -> Result<Vec<MigrationRow>> {
    let mut stmt = conn.prepare(
        "SELECT version, description, installed_on, success, checksum, execution_time
         FROM _sqlx_migrations
         ORDER BY version",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
        ))
    })?;

    rows.collect()
}

fn main() -> Result<()> {
    let source_path = "migrations-history-source.db";
    let encrypted_path = "migrations-history-encrypted.db";
    let password = "laboratory-migrations-password";

    // Никогда не перезаписываем существующие тестовые базы.
    if Path::new(source_path).exists() || Path::new(encrypted_path).exists() {
        println!("Тестовые файлы уже существуют. Ничего не изменено.");
        return Ok(());
    }

    // 1. Создаём таблицу по схеме SQLx 0.8.6.
    let source = Connection::open(source_path)?;

    source.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS _sqlx_migrations (
            version BIGINT PRIMARY KEY,
            description TEXT NOT NULL,
            installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            success BOOLEAN NOT NULL,
            checksum BLOB NOT NULL,
            execution_time BIGINT NOT NULL
        );
        "#,
    )?;

    let descriptions = [
        "create app metadata table",
        "create diary entries table",
        "create tags table",
        "atomic tag changes via SQLite triggers",
        "create preferences snapshot table",
        "create calendar date colors table",
    ];

    // Контрольные суммы здесь искусственные: проверяем точность переноса,
    // а не соответствие настоящим SQL-файлам миграций.
    for (index, description) in descriptions.iter().enumerate() {
        let version = (index + 1) as i64;
        let checksum = vec![version as u8; 32];

        source.execute(
            "INSERT INTO _sqlx_migrations
             (version, description, success, checksum, execution_time)
             VALUES (?1, ?2, 1, ?3, 1000)",
            params![version, description, checksum],
        )?;
    }

    let original_history = read_history(&source)?;
    assert_eq!(original_history.len(), 6);

    println!("1. Создана история из шести тестовых миграций.");

    drop(source);

    // 2. Переносим базу в SQLCipher.
    let source = Connection::open(source_path)?;

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
    drop(source);

    println!("2. Создана зашифрованная копия.");

    // 3. Открываем зашифрованную базу с правильным ключом.
    let encrypted = Connection::open(encrypted_path)?;
    encrypted.pragma_update(None, "key", password)?;

    let encrypted_history = read_history(&encrypted)?;

    // Сравниваем все поля, включая байты контрольных сумм.
    assert_eq!(original_history, encrypted_history);

    println!("3. Все шесть записей миграций совпадают побайтово.");

    // 4. Проверяем отсутствие незавершённых миграций.
    let failed_count: i64 = encrypted.query_row(
        "SELECT COUNT(*) FROM _sqlx_migrations WHERE success = 0",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(failed_count, 0);

    println!("4. Незавершённых миграций нет.");

    drop(encrypted);

    // 5. Убеждаемся, что исходная база не изменилась.
    let original = Connection::open(source_path)?;

    assert_eq!(read_history(&original)?, original_history);

    println!("5. Исходная история миграций не изменилась.");
    println!("Тест истории миграций завершён успешно!");

    Ok(())
}
