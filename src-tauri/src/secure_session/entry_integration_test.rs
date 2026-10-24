use super::SecureSession;
use crate::entry_repository::{insert_entry, Entry};
use rusqlite::Connection;
use std::sync::Arc;

#[test]
fn entry_repository_respects_session_lock() {
    // Только SQLite в оперативной памяти.
    // Настоящий файл дневника не открывается.
    let conn = Connection::open_in_memory().unwrap();

    conn.execute_batch(
        "
        CREATE TABLE entries (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            date TEXT NOT NULL,
            tags TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        ",
    )
    .unwrap();

    let session = Arc::new(SecureSession::new());

    let entry = Entry {
        id: "integration-test-1".to_string(),
        title: "Тестовая запись".to_string(),
        content: "Искусственные данные".to_string(),
        date: "2026-01-01".to_string(),
        tags: vec!["тест".to_string()],
        created_at: "2026-01-01T10:00:00Z".to_string(),
        updated_at: "2026-01-01T10:00:00Z".to_string(),
    };

    // Заблокированная сессия запрещает запись.
    assert!(session
        .with_connection(|db| insert_entry(db, &entry))
        .is_err());

    // Разблокируем сессию тестовым соединением.
    session.unlock_with_connection(conn).unwrap();

    // Создаём запись через защищённую сессию.
    session
        .with_connection(|db| insert_entry(db, &entry))
        .unwrap();

    // Проверяем, что запись действительно сохранена.
    let count: i64 = session
        .with_connection(|db| {
            db.query_row(
                "SELECT COUNT(*) FROM entries WHERE id = ?1",
                [&entry.id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())
        })
        .unwrap();

    assert_eq!(count, 1);

    // Блокируем дневник.
    session.lock_and_wait().unwrap();

    // После блокировки доступ к SQLite запрещён.
    assert!(session
        .with_connection(|db| insert_entry(db, &entry))
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