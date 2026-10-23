use rusqlite::{params, Connection};
use sqlx::{ConnectOptions, Connection as SqlxConnection, Row, sqlite::SqliteConnectOptions};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = "sqlx-compat-source.db";
    let encrypted_path = "sqlx-compat-encrypted.db";
    let password = "laboratory-password-sqlx";

    // Не перезаписываем существующие базы и служебные файлы.
    for base in [source_path, encrypted_path] {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let path = format!("{base}{suffix}");

            if Path::new(&path).exists() {
                println!("Файл {path} уже существует. Ничего не изменено.");
                return Ok(());
            }
        }
    }

    // Создаём искусственную базу SQLite.
    let source = Connection::open(source_path)?;

    source.execute_batch(
        "
        CREATE TABLE entries (
            id TEXT PRIMARY KEY NOT NULL,
            content TEXT NOT NULL
        );

        INSERT INTO entries (id, content)
        VALUES ('sqlx-test', 'Искусственная секретная запись');
        ",
    )?;

    drop(source);

    // Экспортируем её в SQLCipher.
    let original = Connection::open(source_path)?;

    original.execute(
        "ATTACH DATABASE ?1 AS encrypted KEY ?2",
        params![encrypted_path, password],
    )?;

    original.query_row(
        "SELECT sqlcipher_export('encrypted')",
        [],
        |_row| Ok(()),
    )?;

    original.execute_batch("DETACH DATABASE encrypted;")?;
    drop(original);

    println!("1. Искусственная база зашифрована через rusqlite.");

    // Проверяем, что файл не имеет открытого заголовка SQLite.
    let header = std::fs::read(encrypted_path)?;

    assert!(header.len() >= 16);
    assert_ne!(&header[..16], b"SQLite format 3\0");

    println!("2. Зашифрованный заголовок подтверждён.");

    // SQLx формирует PRAGMA напрямую, поэтому значение заключаем
    // в SQL-кавычки. Пароль здесь фиксированный и лабораторный.
    let quoted_password = format!("'{}'", password.replace('\'', "''"));

    let options = SqliteConnectOptions::new()
        .filename(encrypted_path)
        .read_only(true)
        .create_if_missing(false)
        .pragma("key", quoted_password);

    let mut connection = options.connect().await?;

    let row = sqlx::query(
        "SELECT content FROM entries WHERE id = 'sqlx-test'",
    )
    .fetch_one(&mut connection)
    .await?;

    let content: String = row.try_get("content")?;

    assert_eq!(content, "Искусственная секретная запись");

    println!("3. SQLx прочитал данные с правильным паролем.");

    connection.close().await?;

    // Проверяем неправильный пароль.
    let wrong_options = SqliteConnectOptions::new()
        .filename(encrypted_path)
        .read_only(true)
        .create_if_missing(false)
        .pragma("key", "'wrong-password'");

    match wrong_options.connect().await {
        Ok(mut wrong_connection) => {
            let result = sqlx::query("SELECT COUNT(*) FROM entries")
                .fetch_one(&mut wrong_connection)
                .await;

            assert!(
                result.is_err(),
                "Неправильный пароль не должен открывать данные"
            );

            wrong_connection.close().await?;
        }
        Err(_) => {
            // Отказ при подключении тоже считается успешной защитой.
        }
    }

    println!("4. Неправильный пароль не открыл данные.");
    println!("Тест совместимости SQLCipher + SQLx успешно пройден!");

    Ok(())
}
