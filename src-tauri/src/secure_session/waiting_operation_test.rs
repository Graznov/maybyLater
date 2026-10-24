use super::{SecureSession, SessionStatus};
use rusqlite::Connection;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn waiting_operation_is_cancelled_during_lock() {
    let session = Arc::new(SecureSession::new());

    // Только искусственная SQLite-база в памяти.
    session
        .unlock_with_connection(Connection::open_in_memory().unwrap())
        .unwrap();

    let (first_started_tx, first_started_rx) = mpsc::channel();
    let (release_first_tx, release_first_rx) = mpsc::channel();

    let first_session = Arc::clone(&session);

    // Первая операция удерживает SQLite-соединение.
    let first = thread::spawn(move || {
        first_session
            .with_connection(|conn| {
                first_started_tx.send(()).unwrap();
                release_first_rx.recv().unwrap();

                conn.execute("CREATE TABLE test (value INTEGER)", [])
                    .map_err(|e| e.to_string())?;

                Ok(())
            })
            .unwrap();
    });

    first_started_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap();

    let (second_started_tx, second_started_rx) = mpsc::channel();
    let second_session = Arc::clone(&session);

    // Вторая операция попытается получить занятое соединение.
    let second = thread::spawn(move || {
        second_started_tx.send(()).unwrap();

        second_session.with_connection(|conn| {
            conn.execute("INSERT INTO test VALUES (42)", [])
                .map_err(|e| e.to_string())?;

            Ok(())
        })
    });

    second_started_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap();

    // Убеждаемся, что вторая операция получила разрешение
    // и ожидает освобождения SQLite Mutex.
    let start = Instant::now();

    loop {
        let active = session.state.lock().unwrap().active_operations;

        if active == 2 {
            break;
        }

        assert!(
            start.elapsed() < Duration::from_secs(5),
            "Вторая операция не начала ожидание"
        );

        thread::yield_now();
    }

    let lock_session = Arc::clone(&session);

    let locker = thread::spawn(move || {
        lock_session.lock_and_wait().unwrap();
    });

    // Дожидаемся перехода в Locking.
    let start = Instant::now();

    while session.status().unwrap() != SessionStatus::Locking {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "Сессия не перешла в Locking"
        );

        thread::yield_now();
    }

    // Разрешаем первой операции завершиться.
    release_first_tx.send(()).unwrap();

    first.join().unwrap();

    // Ожидающая операция должна быть отменена.
    let second_result = second.join().unwrap();
    assert!(second_result.is_err());

    locker.join().unwrap();

    assert_eq!(session.status().unwrap(), SessionStatus::Locked);
}