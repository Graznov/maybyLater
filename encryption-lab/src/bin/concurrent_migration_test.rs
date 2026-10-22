
use rusqlite::{params, Connection, ErrorCode, Result};
use std::path::Path;
use std::time::Duration;

fn main() -> Result<()> {
    let source_path = "concurrent-source.db";
    let encrypted_path = "concurrent-encrypted.db";
    let password = "laboratory-concurrent-password";

    // Не изменяем файлы предыдущих экспериментов.
    for path in [
        source_path,
        "concurrent-source.db-wal",
        "concurrent-source.db-shm",
        encrypted_path,
        "concurrent-encrypted.db-wal",
        "concurrent-encrypted.db-shm",
    ] {
        if Path::new(path).exists() {
            println!("Файл {path} уже существует. Ничего не изменено.");
            return Ok(());
        }
    }

    // 1. Создаём тестовую базу.
    let setup = Connection::open(source_path)?;

    setup.execute_batch(
        "
        PRAGMA journal_mode = WAL;

        CREATE TABLE entries (
            id TEXT PRIMARY KEY NOT NULL,
            content TEXT NOT NULL
        );

        INSERT INTO entries (id, content)
        VALUES ('entry-1', 'Первая запись');
        ",
    )?;

    drop(setup);

    println!("1. Тестовая база создана.");

    // 2. Начинаем незавершённую транзакцию записи.
    let writer = Connection::open(source_path)?;
    writer.execute_batch("BEGIN IMMEDIATE;")?;

    writer.execute(
        "INSERT INTO entries (id, content) VALUES (?1, ?2)",
        params!["entry-2", "Запись во время миграции"],
    )?;

    println!("2. Вторая запись добавлена в незавершённую транзакцию.");

    // 3. Пробуем начать миграцию через другое соединение.
    let migrator = Connection::open(source_path)?;
    migrator.busy_timeout(Duration::from_millis(500))?;

    let attempt = (|| -> Result<()> {
        migrator.execute(
            "ATTACH DATABASE ?1 AS encrypted KEY ?2",
            params![encrypted_path, password],
        )?;

        migrator.query_row(
            "SELECT sqlcipher_export('encrypted')",
            [],
            |_row| Ok(()),
        )?;

        Ok(())
    })();

    match attempt {
        Err(rusqlite::Error::SqliteFailure(error, _))
            if error.code == ErrorCode::DatabaseBusy
                || error.code == ErrorCode::DatabaseLocked =>
        {
            println!("3. Экспорт остановлен из-за блокировки базы.");
        }
        Err(error) => {
            println!("3. Экспорт завершился другой ошибкой: {error}");
            return Err(error);
        }
        Ok(()) => {
            println!(
                "3. Экспорт завершился при незавершённой записи. \
                 Этот сценарий требует дополнительной защиты!"
            );
        }
    }

    // 4. Завершаем транзакцию записи.
    writer.execute_batch("COMMIT;")?;
    drop(writer);

    println!("4. Вторая запись сохранена.");

    // 5. Проверяем, что исходная база содержит обе записи.
    let source_count: i64 =
        migrator.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))?;

    assert_eq!(source_count, 2);

    println!("5. Исходная база содержит обе записи.");

    // 6. Проверяем состояние зашифрованного файла.
    // Если экспорт успел завершиться, сравним количество записей.
    if Path::new(encrypted_path).exists() {
        let encrypted = Connection::open(encrypted_path)?;
        encrypted.pragma_update(None, "key", password)?;

        let result: Result<i64> =
            encrypted.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0));

        match result {
            Ok(count) => {
                println!("6. В зашифрованной копии записей: {count}.");

                if count != source_count {
                    println!(
                        "ВНИМАНИЕ: зашифрованная копия не содержит \
                         все последние записи!"
                    );
                }
            }
            Err(error) => {
                println!("6. Зашифрованная копия неполная: {error}");
            }
        }
    }

    println!("Эксперимент с конкурентной записью завершён.");

    Ok(())
}
