use super::SecureSession;
use crate::clear_user_data;
use rusqlite::Connection;
use std::sync::Arc;

#[test]
fn reset_respects_session_lock() {
    let session = Arc::new(SecureSession::new());

    // Создаём исключительно временную базу в оперативной памяти.
    let conn = Connection::open_in_memory().unwrap();

    conn.execute_batch(
        "
        CREATE TABLE entries (
            id TEXT PRIMARY KEY,
            title TEXT,
            content TEXT,
            date TEXT,
            tags TEXT,
            created_at TEXT,
            updated_at TEXT
        );

        CREATE TABLE tags (
            id TEXT PRIMARY KEY,
            name TEXT,
            color TEXT
        );

        CREATE TABLE preferences (
            key TEXT PRIMARY KEY,
            value TEXT
        );

        CREATE TABLE calendar_colors (
            date TEXT PRIMARY KEY,
            color TEXT
        );

        CREATE TABLE app_metadata (
            key TEXT PRIMARY KEY,
            value TEXT
        );

        INSERT INTO entries
        VALUES ('e1', 'Тестовая запись', 'Текст', '2026-01-01', '[]', 'now', 'now');

        INSERT INTO tags
        VALUES ('t1', 'Тестовый тег', 'red');

        INSERT INTO preferences
        VALUES ('preferences.v1', '{}');

        INSERT INTO calendar_colors
        VALUES ('2026-01-01', 'red');

        INSERT INTO app_metadata
        VALUES ('test-marker', 'preserved');
        ",
    )
    .unwrap();

    // Пока сессия заблокирована, сброс запрещён.
    assert!(session
        .with_connection(clear_user_data)
        .is_err());

    // Разблокируем сессию с тестовым соединением.
    session.unlock_with_connection(conn).unwrap();

    // До сброса все четыре пользовательские таблицы содержат данные.
    session
        .with_connection(|db| {
            for table in ["entries", "tags", "preferences", "calendar_colors"] {
                let count: i64 = db
                    .query_row(
                        &format!("SELECT COUNT(*) FROM {table}"),
                        [],
                        |row| row.get(0),
                    )
                    .map_err(|e| e.to_string())?;

                assert_eq!(count, 1, "{table}");
            }

            Ok(())
        })
        .unwrap();

    // Выполняем сброс через защищённую сессию.
    session.with_connection(clear_user_data).unwrap();

    // Пользовательские данные удалены, служебные данные сохранены.
    session
        .with_connection(|db| {
            for table in ["entries", "tags", "preferences", "calendar_colors"] {
                let count: i64 = db
                    .query_row(
                        &format!("SELECT COUNT(*) FROM {table}"),
                        [],
                        |row| row.get(0),
                    )
                    .map_err(|e| e.to_string())?;

                assert_eq!(count, 0, "{table}");
            }

            let marker: String = db
                .query_row(
                    "SELECT value FROM app_metadata WHERE key = ?1",
                    ["test-marker"],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;

            assert_eq!(marker, "preserved");

            Ok(())
        })
        .unwrap();

    // После блокировки любые обращения к соединению запрещены.
    session.lock_and_wait().unwrap();

    assert!(session
        .with_connection(clear_user_data)
        .is_err());

    assert!(session
        .with_connection(|db| {
            db.query_row("SELECT COUNT(*) FROM entries", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|e| e.to_string())
        })
        .is_err());
}