use super::SecureSession;
use crate::sqlcipher_connection::open_verified_database;
use std::path::Path;
use std::sync::Arc;

#[test]
fn verified_sqlcipher_connection_respects_session_lock() {
    let session = Arc::new(SecureSession::new());

    // До разблокировки доступ запрещён.
    assert!(session.with_connection(|_| Ok(())).is_err());

    // Используем только изолированную тестовую базу.
    let path = Path::new(
        "../../sqlcipher-build-test/test-data/encrypted-test.db"
    );

    let conn = open_verified_database(path, "correct-test-password")
        .expect("SQLCipher должен принять правильный пароль");

    session.unlock_with_connection(conn).unwrap();

    // После разблокировки тестовая запись доступна.
    let message: String = session
        .with_connection(|db| {
            db.query_row(
                "SELECT message FROM test LIMIT 1",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())
        })
        .unwrap();

    assert_eq!(message, "Secret test message");

    // Блокировка закрывает соединение.
    session.lock_and_wait().unwrap();

    // После блокировки чтение запрещено.
    assert!(session
        .with_connection(|db| {
            db.query_row(
                "SELECT message FROM test LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .map_err(|e| e.to_string())
        })
        .is_err());
}
