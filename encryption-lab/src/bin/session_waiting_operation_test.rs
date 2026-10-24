use rusqlite::Connection;
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionStatus {
    Locked,
    Locking,
    Unlocked,
}

struct SessionState {
    status: SessionStatus,
    active_operations: usize,
}

struct SecureSession {
    state: Mutex<SessionState>,
    changed: Condvar,
    connection: Mutex<Option<Connection>>,
}

struct OperationPermit {
    session: Arc<SecureSession>,
}

impl Drop for OperationPermit {
    fn drop(&mut self) {
        let mut state = self.session.state.lock().unwrap();
        state.active_operations -= 1;
        self.session.changed.notify_all();
    }
}

impl SecureSession {
    fn new() -> Self {
        Self {
            state: Mutex::new(SessionState {
                status: SessionStatus::Locked,
                active_operations: 0,
            }),
            changed: Condvar::new(),
            connection: Mutex::new(None),
        }
    }

    fn unlock(&self) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;

        if state.status != SessionStatus::Locked {
            return Err("Разблокировка сейчас запрещена".into());
        }

        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE entries (
                id TEXT PRIMARY KEY NOT NULL,
                content TEXT NOT NULL
            );",
        )
        .map_err(|e| e.to_string())?;

        let mut connection = self.connection.lock().map_err(|e| e.to_string())?;
        *connection = Some(conn);

        state.status = SessionStatus::Unlocked;
        self.changed.notify_all();

        Ok(())
    }

    fn begin_operation(self: &Arc<Self>) -> Result<OperationPermit, String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;

        if state.status != SessionStatus::Unlocked {
            return Err("Дневник заблокирован".into());
        }

        state.active_operations += 1;
        self.changed.notify_all();

        Ok(OperationPermit {
            session: Arc::clone(self),
        })
    }

    fn with_connection<T>(
        self: &Arc<Self>,
        operation: impl FnOnce(&Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let _permit = self.begin_operation()?;

        // Ожидаем освобождения SQLite-соединения.
        let connection = self.connection.lock().map_err(|e| e.to_string())?;

        // Повторно проверяем состояние после ожидания.
        let state = self.state.lock().map_err(|e| e.to_string())?;

        if state.status != SessionStatus::Unlocked {
            return Err("Операция отменена: дневник блокируется".into());
        }

        let conn = connection
            .as_ref()
            .ok_or("SQLite-соединение отсутствует")?;

        // Убираем блокировку состояния до выполнения SQL.
        // Разрешение _permit остаётся активным.
        drop(state);

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
        self.changed.notify_all();

        while state.active_operations > 0 {
            state = self.changed.wait(state).map_err(|e| e.to_string())?;
        }

        let mut connection = self.connection.lock().map_err(|e| e.to_string())?;
        *connection = None;

        state.status = SessionStatus::Locked;
        self.changed.notify_all();

        Ok(())
    }

    fn status(&self) -> SessionStatus {
        self.state.lock().unwrap().status
    }

    fn active_operations(&self) -> usize {
        self.state.lock().unwrap().active_operations
    }
}

fn main() -> Result<(), String> {
    let session = Arc::new(SecureSession::new());

    session.unlock()?;

    session.with_connection(|conn| {
        conn.execute(
            "INSERT INTO entries (id, content) VALUES ('1', 'Искусственная запись')",
            [],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })?;

    println!("1. Искусственная SQLite-база подготовлена.");

    let (first_started_tx, first_started_rx) = mpsc::channel::<()>();
    let (finish_first_tx, finish_first_rx) = mpsc::channel::<()>();

    let first_session = Arc::clone(&session);

    let first = thread::spawn(move || {
        first_session.with_connection(|conn| {
            first_started_tx.send(()).unwrap();

            // Первая операция удерживает SQLite Mutex.
            finish_first_rx.recv().unwrap();

            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
                .map_err(|e| e.to_string())?;

            assert_eq!(count, 1);
            Ok(())
        })
    });

    first_started_rx.recv().unwrap();
    println!("2. Первая операция удерживает SQLite-соединение.");

    // Вторая операция получает разрешение до блокировки,
    // но не может пока получить SQLite Mutex.
    let second_session = Arc::clone(&session);
    let (second_result_tx, second_result_rx) = mpsc::channel();

    let second = thread::spawn(move || {
        let result = second_session.with_connection(|conn| {
            conn.execute(
                "INSERT INTO entries (id, content) VALUES ('2', 'Поздняя запись')",
                [],
            )
            .map_err(|e| e.to_string())?;

            Ok(())
        });

        second_result_tx.send(result).unwrap();
    });

    // Ждём, пока обе операции получат разрешения.
    {
        let mut state = session.state.lock().unwrap();

        while state.active_operations != 2 {
            state = session.changed.wait(state).unwrap();
        }
    }

    println!("3. Вторая операция ожидает SQLite-соединение.");

    let locker_session = Arc::clone(&session);
    let (locked_tx, locked_rx) = mpsc::channel::<Result<(), String>>();

    let locker = thread::spawn(move || {
        locked_tx.send(locker_session.lock_and_wait()).unwrap();
    });

    // Ждём перехода в Locking.
    {
        let mut state = session.state.lock().unwrap();

        while state.status != SessionStatus::Locking {
            state = session.changed.wait(state).unwrap();
        }
    }

    assert_eq!(session.status(), SessionStatus::Locking);
    assert_eq!(session.active_operations(), 2);

    println!("4. Дневник перешёл в Locking.");

    // После запроса блокировки новые операции запрещены.
    assert!(session.with_connection(|_| Ok(())).is_err());

    // Первая операция может завершиться.
    finish_first_tx.send(()).unwrap();
    first.join().unwrap()?;

    // Вторая операция должна отказаться от SQL-запроса.
    let second_result = second_result_rx.recv().unwrap();
    assert!(second_result.is_err());

    second.join().unwrap();

    locked_rx.recv().unwrap()?;
    locker.join().unwrap();

    assert_eq!(session.status(), SessionStatus::Locked);
    assert_eq!(session.active_operations(), 0);

    println!("5. Ожидающая операция отменена без записи.");
    println!("6. Соединение закрыто, сессия заблокирована.");

    session.unlock()?;

    let count: i64 = session.with_connection(|conn| {
        conn.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
            .map_err(|e| e.to_string())
    })?;

    // Новая база в памяти создаётся пустой.
    assert_eq!(count, 0);

    session.lock_and_wait()?;

    println!("7. Повторная разблокировка работает.");
    println!("Тест ожидающей операции успешно пройден!");

    Ok(())
}