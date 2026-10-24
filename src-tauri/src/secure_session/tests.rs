use super::{SecureSession, SessionStatus};
use rusqlite::Connection;
use std::sync::Arc;

#[test]
fn session_starts_locked_and_relocks() {
    let session = Arc::new(SecureSession::new());

    // Новая сессия изначально заблокирована.
    assert_eq!(session.status().unwrap(), SessionStatus::Locked);

    // Пока дневник заблокирован, SQLite-операции запрещены.
    let result = session.with_connection(|_| Ok(()));
    assert!(result.is_err());

    // Используем исключительно искусственную SQLite-базу в памяти.
    let connection = Connection::open_in_memory().unwrap();

    session.unlock_with_connection(connection).unwrap();

    assert_eq!(session.status().unwrap(), SessionStatus::Unlocked);

    // После разблокировки можно выполнять SQLite-запросы.
    let value = session
        .with_connection(|conn| {
            conn.query_row("SELECT 42", [], |row| row.get::<_, i64>(0))
                .map_err(|e| e.to_string())
        })
        .unwrap();

    assert_eq!(value, 42);

    // Блокировка закрывает соединение.
    session.lock_and_wait().unwrap();

    assert_eq!(session.status().unwrap(), SessionStatus::Locked);

    // После блокировки новые операции снова запрещены.
    assert!(session.with_connection(|_| Ok(())).is_err());
}

#[test]
fn session_can_unlock_again_after_lock() {
    let session = Arc::new(SecureSession::new());

    // Первая разблокировка — только SQLite в памяти.
    session
        .unlock_with_connection(Connection::open_in_memory().unwrap())
        .unwrap();

    session
        .with_connection(|conn| {
            conn.execute("CREATE TABLE notes (text TEXT)", [])
                .map_err(|e| e.to_string())?;
            conn.execute("INSERT INTO notes VALUES ('test')", [])
                .map_err(|e| e.to_string())?;
            Ok(())
        })
        .unwrap();

    session.lock_and_wait().unwrap();

    // Повторная разблокировка создаёт новое соединение.
    session
        .unlock_with_connection(Connection::open_in_memory().unwrap())
        .unwrap();

    assert_eq!(session.status().unwrap(), SessionStatus::Unlocked);

    // Данные старой базы в памяти недоступны после её закрытия.
    let result = session.with_connection(|conn| {
        conn.query_row("SELECT text FROM notes", [], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())
    });

    assert!(result.is_err());

    session.lock_and_wait().unwrap();
}

#[test]
fn lock_waits_for_active_operation() {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    let session = Arc::new(SecureSession::new());

    session
        .unlock_with_connection(Connection::open_in_memory().unwrap())
        .unwrap();

    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();

    let worker_session = Arc::clone(&session);

    let worker = thread::spawn(move || {
        worker_session
            .with_connection(|conn| {
                conn.execute("CREATE TABLE test (value INTEGER)", [])
                    .map_err(|e| e.to_string())?;

                started_tx.send(()).unwrap();

                // Имитируем длительную SQLite-операцию.
                release_rx.recv().unwrap();

                conn.execute("INSERT INTO test VALUES (42)", [])
                    .map_err(|e| e.to_string())?;

                Ok(())
            })
            .unwrap();
    });

    // Дожидаемся начала операции.
    started_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap();

    let lock_session = Arc::clone(&session);

    let locker = thread::spawn(move || {
        lock_session.lock_and_wait().unwrap();
    });

    // Дожидаемся перехода в Locking.
    let start = std::time::Instant::now();

    while session.status().unwrap() != SessionStatus::Locking {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "Сессия не перешла в Locking"
        );

        thread::yield_now();
    }

    // Пока операция выполняется, соединение не должно закрыться.
    assert_eq!(session.status().unwrap(), SessionStatus::Locking);

    // Разрешаем активной операции завершиться.
    release_tx.send(()).unwrap();

    worker.join().unwrap();
    locker.join().unwrap();

    // Только после завершения операции сессия блокируется.
    assert_eq!(session.status().unwrap(), SessionStatus::Locked);

    assert!(session.with_connection(|_| Ok(())).is_err());
}