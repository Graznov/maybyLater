
use super::SecureSession;
use crate::{apply_backup, BackupEntry, BackupPayload, BackupTag};
use rusqlite::Connection;
use std::sync::Arc;

#[test]
fn backup_restore_respects_session_lock() {
    // Исключительно SQLite в памяти — никаких файлов дневника.
    let conn = Connection::open_in_memory().unwrap();

    conn.execute_batch(
        "
        CREATE TABLE entries (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            date TEXT NOT NULL,
            tags TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE tags (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            color TEXT NOT NULL
        );

        CREATE TABLE preferences (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE calendar_colors (
            date TEXT PRIMARY KEY,
            color TEXT NOT NULL
        );
        ",
    )
    .unwrap();

    let session = Arc::new(SecureSession::new());

    let backup = BackupPayload {
        entries: vec![BackupEntry {
            id: "test-entry-1".into(),
            title: "Тестовая запись".into(),
            content: "Искусственные данные".into(),
            date: "2026-01-01".into(),
            tags: vec!["test-tag-1".into()],
            created_at: "2026-01-01T10:00:00Z".into(),
            updated_at: "2026-01-01T10:00:00Z".into(),
        }],
        tags: vec![BackupTag {
            id: "test-tag-1".into(),
            name: "Тестовый тег".into(),
            color: "#00AA00".into(),
        }],
        preferences: None,
        calendar_colors: None,
    };

    // Заблокированная сессия не должна разрешать восстановление.
    assert!(session
        .with_connection(|db| apply_backup(db, &backup))
        .is_err());

    session.unlock_with_connection(conn).unwrap();

    // Восстановление через защищённую сессию.
    session
        .with_connection(|db| apply_backup(db, &backup))
        .unwrap();

    // Проверяем, что восстановлены запись и тег.
    session
        .with_connection(|db| {
            let title: String = db
                .query_row(
                    "SELECT title FROM entries WHERE id = ?1",
                    ["test-entry-1"],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;

            let tag_name: String = db
                .query_row(
                    "SELECT name FROM tags WHERE id = ?1",
                    ["test-tag-1"],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;

            assert_eq!(title, "Тестовая запись");
            assert_eq!(tag_name, "Тестовый тег");

            Ok(())
        })
        .unwrap();

    // Блокируем сессию: соединение закрывается.
    session.lock_and_wait().unwrap();

    // Повторное восстановление после блокировки запрещено.
    assert!(session
        .with_connection(|db| apply_backup(db, &backup))
        .is_err());
}
