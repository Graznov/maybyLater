
use super::SecureSession;
use rusqlite::Connection;
use std::sync::Arc;

#[test]
fn transactions_respect_session_lock() {
    // Используем исключительно SQLite в оперативной памяти.
    let conn = Connection::open_in_memory().unwrap();

    conn.execute_batch(
        "
        CREATE TABLE test_entries (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL
        );
        ",
    )
    .unwrap();

    let session = Arc::new(SecureSession::new());

    // До разблокировки транзакции запрещены.
    assert!(session
        .with_connection(|db| {
            let tx = db.transaction().map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT INTO test_entries (id, title) VALUES (1, 'Первая запись')",
                [],
            )
            .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())
        })
        .is_err());

    session.unlock_with_connection(conn).unwrap();

    // Успешная транзакция.
    session
        .with_connection(|db| {
            let tx = db.transaction().map_err(|e| e.to_string())?;

            tx.execute(
                "INSERT INTO test_entries (id, title) VALUES (1, 'Первая запись')",
                [],
            )
            .map_err(|e| e.to_string())?;

            tx.commit().map_err(|e| e.to_string())
        })
        .unwrap();

    // Проверяем откат при ошибке.
    let result = session.with_connection(|db| {
        let tx = db.transaction().map_err(|e| e.to_string())?;

        tx.execute(
            "INSERT INTO test_entries (id, title) VALUES (2, 'Временная запись')",
            [],
        )
        .map_err(|e| e.to_string())?;

        // Повторяющийся ID вызывает ошибку UNIQUE.
        tx.execute(
            "INSERT INTO test_entries (id, title) VALUES (1, 'Дубликат')",
            [],
        )
        .map_err(|e| e.to_string())?;

        tx.commit().map_err(|e| e.to_string())
    });

    assert!(result.is_err());

    // После ошибки должна сохраниться только первая запись.
    let entries: Vec<(i64, String)> = session
        .with_connection(|db| {
            let mut stmt = db
                .prepare("SELECT id, title FROM test_entries ORDER BY id")
                .map_err(|e| e.to_string())?;

            let rows = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(|e| e.to_string())?;

            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|e| e.to_string())
        })
        .unwrap();

    assert_eq!(entries, vec![(1, "Первая запись".to_string())]);

    // После блокировки любые новые транзакции запрещены.
    session.lock_and_wait().unwrap();

    assert!(session
        .with_connection(|db| {
            let tx = db.transaction().map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT INTO test_entries (id, title) VALUES (3, 'Запрещённая запись')",
                [],
            )
            .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())
        })
        .is_err());
}
