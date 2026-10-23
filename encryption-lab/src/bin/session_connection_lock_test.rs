use rusqlite::{params, Connection};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionStatus {
    Locked,
    Unlocked,
    Locking,
}

struct SessionState {
    status: SessionStatus,
    active_operations: usize,
    connection: Option<Connection>,
}

struct SecureSession {
    state: Mutex<SessionState>,
    changed: Condvar,
}

impl SecureSession {
    fn new() -> Self {
        Self {
            state: Mutex::new(SessionState {
                status: SessionStatus::Locked,
                active_operations: 0,
                connection: None,
            }),
            changed: Condvar::new(),
        }
    }

    fn unlock(&self) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;

        if state.status == SessionStatus::Locking {
            return Err("Блокировка ещё выполняется".into());
        }

        if state.status == SessionStatus::Unlocked {
            return Err("Сессия уже разблокирована".into());
        }

        // Только лабораторная SQLite-база в памяти.
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;

        conn.execute_batch(
            "
            CREATE TABLE entries (
                id TEXT PRIMARY KEY NOT NULL,
                content TEXT NOT NULL
            );
            ",
        )
        .map_err(|e| e.to_string())?;

        state.connection = Some(conn);
        state.status = SessionStatus::Unlocked;

        Ok(())
    }

    fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let state = self.state.lock().map_err(|e| e.to_string())?;

        if state.status != SessionStatus::Unlocked {
            return Err("Дневник заблокирован".into());
        }

        let conn = state
            .connection
            .as_ref()
            .ok_or("SQLite-соединение отсутствует")?;

        // Mutex удерживается на протяжении операции.
        // Поэтому блокировка не сможет закрыть соединение
        // посреди SQL-запроса.
        operation(conn)
    }

    fn lock_and_wait(&self) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;

        if state.status == SessionStatus::Locked {
            return Ok(());
        }

        if state.status == SessionStatus::Locking {
            return Err("Блокировка уже выполняется".into());
        }

        state.status = SessionStatus::Locking;

        while state.active_operations > 0 {
            state = self.changed.wait(state).map_err(|e| e.to_string())?;
        }

        // Закрываем соединение перед переходом в Locked.
        state.connection.take();

        state.status = SessionStatus::Locked;
        self.changed.notify_all();

        Ok(())
    }

    fn status(&self) -> SessionStatus {
        self.state.lock().unwrap().status
    }
}

fn main() -> Result<(), String> {
    let session = Arc::new(SecureSession::new());

    // 1. До разблокировки соединения нет.
    assert_eq!(session.status(), SessionStatus::Locked);

    assert!(session
        .with_connection(|conn| {
            conn.execute("INSERT INTO entries VALUES (?1, ?2)", params!["1", "test"])
                .map_err(|e| e.to_string())?;
            Ok(())
        })
        .is_err());

    println!("1. Заблокированная сессия запрещает SQLite-доступ.");

    // 2. Разблокируем лабораторную базу.
    session.unlock()?;

    session.with_connection(|conn| {
        conn.execute(
            "INSERT INTO entries (id, content) VALUES (?1, ?2)",
            params!["1", "Искусственная запись"],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    })?;

    println!("2. Запись в SQLite после разблокировки разрешена.");

    // 3. Проверяем чтение.
    let content: String = session.with_connection(|conn| {
        conn.query_row(
            "SELECT content FROM entries WHERE id = ?1",
            ["1"],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())
    })?;

    assert_eq!(content, "Искусственная запись");

    println!("3. Чтение SQLite после разблокировки разрешено.");

    // 4. Запускаем операцию в отдельном потоке.
    let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
    let (finish_tx, finish_rx) = std::sync::mpsc::channel::<()>();

    let worker_session = Arc::clone(&session);

    let worker = thread::spawn(move || {
        worker_session.with_connection(|conn| {
            started_tx.send(()).unwrap();

            // Удерживаем операцию активной.
            finish_rx.recv().unwrap();

            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
                .map_err(|e| e.to_string())?;

            assert_eq!(count, 1);

            Ok(())
        })
    });

    started_rx.recv().unwrap();

    // 5. Запрашиваем блокировку из другого потока.
    let locker_session = Arc::clone(&session);

    let locker = thread::spawn(move || locker_session.lock_and_wait());

    // Разрешаем операции завершиться.
    finish_tx.send(()).unwrap();

    worker.join().unwrap()?;
    locker.join().unwrap()?;

    assert_eq!(session.status(), SessionStatus::Locked);

    println!("4. Блокировка дождалась завершения SQLite-операции.");

    // 6. После блокировки SQL-запросы запрещены.
    assert!(session
        .with_connection(|conn| {
            conn.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get::<_, i64>(0))
                .map_err(|e| e.to_string())
        })
        .is_err());

    println!("5. После блокировки SQLite-доступ запрещён.");

    // 7. Повторная разблокировка создаёт новую базу в памяти.
    session.unlock()?;

    let count: i64 = session.with_connection(|conn| {
        conn.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
            .map_err(|e| e.to_string())
    })?;

    assert_eq!(count, 0);

    println!("6. Старое соединение закрыто, новая база пуста.");

    session.lock_and_wait()?;

    assert_eq!(session.status(), SessionStatus::Locked);

    println!("Тест защищённого SQLite-соединения успешно пройден!");

    Ok(())
}