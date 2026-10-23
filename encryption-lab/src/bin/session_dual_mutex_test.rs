use rusqlite::{params, Connection};
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
    // Управляет состоянием сессии.
    state: Mutex<SessionState>,
    changed: Condvar,

    // Отдельно защищает SQLite-соединение.
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

        // Исключительно SQLite в памяти.
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

        Ok(OperationPermit {
            session: Arc::clone(self),
        })
    }

    fn with_connection<T>(
        self: &Arc<Self>,
        operation: impl FnOnce(&Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        // Разрешение получаем ДО ожидания SQLite Mutex.
        // Поэтому блокировка учитывает и ожидающие операции.
        let _permit = self.begin_operation()?;

        let connection = self.connection.lock().map_err(|e| e.to_string())?;

        let conn = connection
            .as_ref()
            .ok_or("SQLite-соединение отсутствует")?;

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

        // Быстро запрещаем новые операции.
        // Для этого НЕ требуется SQLite Mutex.
        state.status = SessionStatus::Locking;
        self.changed.notify_all();

        while state.active_operations > 0 {
            state = self.changed.wait(state).map_err(|e| e.to_string())?;
        }

        // Теперь все разрешения освобождены.
        // Старых операций с соединением больше нет.
        let mut connection = self.connection.lock().map_err(|e| e.to_string())?;
        *connection = None;

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

    // 1. При запуске доступ закрыт.
    assert_eq!(session.status(), SessionStatus::Locked);

    assert!(session
        .with_connection(|conn| {
            conn.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))
                .map_err(|e| e.to_string())
        })
        .is_err());

    println!("1. Заблокированная сессия запрещает SQLite-доступ.");

    // 2. Разблокируем искусственную базу.
    session.unlock()?;

    session.with_connection(|conn| {
        conn.execute(
            "INSERT INTO entries (id, content) VALUES (?1, ?2)",
            params!["test-1", "Искусственная запись"],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    })?;

    println!("2. SQLite-запись после разблокировки разрешена.");

    // 3. Удерживаем SQLite Mutex внутри активной операции.
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (finish_tx, finish_rx) = mpsc::channel::<()>();

    let worker_session = Arc::clone(&session);

    let worker = thread::spawn(move || {
        worker_session.with_connection(|conn| {
            started_tx.send(()).unwrap();

            // Имитируем длительный запрос.
            finish_rx.recv().unwrap();

            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
                .map_err(|e| e.to_string())?;

            assert_eq!(count, 1);

            Ok(())
        })
    });

    started_rx.recv().unwrap();

    // 4. Запрашиваем блокировку, пока операция активна.
    let locker_session = Arc::clone(&session);
let (locked_tx, locked_rx) = mpsc::channel::<Result<(), String>>();
    let locker = thread::spawn(move || {
        let result = locker_session.lock_and_wait();
        locked_tx.send(result).unwrap();
    });

    // Ждём перехода в Locking без sleep.
    {
        let mut state = session.state.lock().unwrap();

        while state.status != SessionStatus::Locking {
            state = session.changed.wait(state).unwrap();
        }

        assert_eq!(state.active_operations, 1);
    }

    println!("3. Состояние Locking установлено при занятом SQLite Mutex.");

    // 5. Новые запросы уже запрещены.
    assert!(session
        .with_connection(|conn| {
            conn.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))
                .map_err(|e| e.to_string())
        })
        .is_err());

    // Повторная разблокировка тоже запрещена.
    assert!(session.unlock().is_err());

    // Старая операция ещё выполняется.
    assert!(locked_rx.try_recv().is_err());

    println!("4. Новые запросы и разблокировка запрещены.");

    // 6. Разрешаем активной операции завершиться.
    finish_tx.send(()).unwrap();

    worker.join().unwrap()?;
    locked_rx.recv().unwrap()?;
    locker.join().unwrap();

    assert_eq!(session.status(), SessionStatus::Locked);

    println!("5. Активная операция завершилась, соединение закрыто.");

    // 7. После блокировки доступ отсутствует.
    assert!(session
        .with_connection(|conn| {
            conn.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))
                .map_err(|e| e.to_string())
        })
        .is_err());

    println!("6. После блокировки SQLite-доступ запрещён.");

    // 8. Повторная разблокировка создаёт новую базу в памяти.
    session.unlock()?;

    let count: i64 = session.with_connection(|conn| {
        conn.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))
            .map_err(|e| e.to_string())
    })?;

    assert_eq!(count, 0);

    session.lock_and_wait()?;

    println!("7. Повторная разблокировка и блокировка работают.");
    println!("Тест двух Mutex успешно пройден!");

    Ok(())
}