use rusqlite::Connection;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Locked,
    Initializing,
    Locking,
    Unlocked,
}

struct SessionState {
    status: SessionStatus,
    active_operations: usize,
}

pub struct SecureSession {
    state: Mutex<SessionState>,
    changed: Condvar,
    connection: Mutex<Option<Connection>>,
}

pub struct InitializationGuard {
    session: Arc<SecureSession>,
}

impl Drop for InitializationGuard {
    fn drop(&mut self) {
        let _ = self.session.cancel_initialization();
    }
}

/// Эксклюзивная резервация уже открытой сессии для смены пароля.
///
/// При создании guard сессия переводится в Locking, ожидает завершения
/// всех активных операций, закрывает старое соединение и переходит
/// в Initializing. Владелец guard обязан установить новое (или повторно
/// открытое старое) соединение через finish().
///
/// Если guard будет уничтожен без finish(), сессия останется Locked.
/// Это безопаснее, чем оставлять неизвестное или потенциально неверное
/// соединение доступным.
pub struct PasswordChangeGuard {
    session: Arc<SecureSession>,
    completed: bool,
}

impl PasswordChangeGuard {
    /// Завершает смену пароля и возвращает сессию в Unlocked.
    ///
    /// Передайте соединение, открытое с новым паролем при успехе,
    /// либо соединение, повторно открытое со старым паролем при откате.
    pub fn finish(mut self, conn: Connection) -> Result<(), String> {
        self.session.finish_initialization(conn)?;
        self.completed = true;
        Ok(())
    }
}

impl Drop for PasswordChangeGuard {
    fn drop(&mut self) {
        if !self.completed {
            let _ = self.session.cancel_initialization();
        }
    }
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
    pub fn new() -> Self {
        Self {
            state: Mutex::new(SessionState {
                status: SessionStatus::Locked,
                active_operations: 0,
            }),
            changed: Condvar::new(),
            connection: Mutex::new(None),
        }
    }

    pub fn status(&self) -> Result<SessionStatus, String> {
        let state = self.state.lock().map_err(|e| e.to_string())?;
        Ok(state.status)
    }

    pub fn begin_initialization(self: &Arc<Self>) -> Result<InitializationGuard, String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        if state.status != SessionStatus::Locked {
            return Err("Создание дневника запрещено: сессия занята".into());
        }
        state.status = SessionStatus::Initializing;
        self.changed.notify_all();
        Ok(InitializationGuard {
            session: Arc::clone(self),
        })
    }

    /// Начинает эксклюзивную операцию над уже открытой базой.
    ///
    /// В отличие от lock_and_wait(), после ожидания сессия не становится
    /// Locked, а резервируется в Initializing для безопасной замены базы.
    pub fn begin_password_change(self: &Arc<Self>) -> Result<PasswordChangeGuard, String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;

        if state.status != SessionStatus::Unlocked {
            return Err("Сменить пароль можно только у разблокированного дневника".into());
        }

        // Запрещаем новые операции. Уже начатые операции завершаются,
        // а ожидающие SQLite Mutex увидят Locking и отменятся.
        state.status = SessionStatus::Locking;
        self.changed.notify_all();

        while state.active_operations > 0 {
            state = self.changed.wait(state).map_err(|e| e.to_string())?;
        }

        // Активных операций больше нет, поэтому соединение можно закрыть.
        let mut connection = self.connection.lock().map_err(|e| e.to_string())?;
        *connection = None;

        state.status = SessionStatus::Initializing;
        self.changed.notify_all();

        Ok(PasswordChangeGuard {
            session: Arc::clone(self),
            completed: false,
        })
    }

    pub fn finish_initialization(&self, conn: Connection) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        if state.status != SessionStatus::Initializing {
            return Err("Сессия не находится в состоянии инициализации".into());
        }
        let mut connection = self.connection.lock().map_err(|e| e.to_string())?;
        *connection = Some(conn);
        state.status = SessionStatus::Unlocked;
        self.changed.notify_all();
        Ok(())
    }

    pub fn cancel_initialization(&self) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        if state.status != SessionStatus::Initializing {
            return Err("Невозможно отменить инициализацию: неверное состояние сессии".into());
        }
        state.status = SessionStatus::Locked;
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

    pub fn with_connection<T>(
        self: &Arc<Self>,
        operation: impl FnOnce(&mut Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let _permit = self.begin_operation()?;

        let mut connection = self.connection.lock().map_err(|e| e.to_string())?;

        // Повторная проверка после ожидания SQLite Mutex.
        let state = self.state.lock().map_err(|e| e.to_string())?;

        if state.status != SessionStatus::Unlocked {
            return Err("Операция отменена: дневник блокируется".into());
        }

        let conn = connection.as_mut().ok_or("SQLite-соединение отсутствует")?;

        drop(state);

        operation(conn)
    }

    pub fn lock_and_wait(&self) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;

        match state.status {
            SessionStatus::Locked => return Ok(()),
            SessionStatus::Initializing => {
                return Err("Нельзя заблокировать дневник во время инициализации".into());
            }
            SessionStatus::Locking => {
                return Err("Блокировка уже выполняется".into());
            }
            SessionStatus::Unlocked => {}
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

    // Пока принимает только готовое соединение.
    // Позже здесь появятся проверка пароля и открытие SQLCipher.
    pub fn unlock_with_connection(&self, conn: Connection) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;

        if state.status != SessionStatus::Locked {
            return Err("Разблокировка сейчас запрещена".into());
        }

        let mut connection = self.connection.lock().map_err(|e| e.to_string())?;
        *connection = Some(conn);

        state.status = SessionStatus::Unlocked;
        self.changed.notify_all();

        Ok(())
    }
}

impl Default for SecureSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod initialization_tests {
    use super::{SecureSession, SessionStatus};

    #[test]
    fn initialization_guard_restores_locked_on_drop() {
        let session = std::sync::Arc::new(SecureSession::new());
        {
            let _guard = session.begin_initialization().unwrap();
            assert_eq!(session.status().unwrap(), SessionStatus::Initializing);
        }
        assert_eq!(session.status().unwrap(), SessionStatus::Locked);
        let _guard = session.begin_initialization().unwrap();
        assert_eq!(session.status().unwrap(), SessionStatus::Initializing);
    }

    #[test]
    fn initialization_reserves_session_and_supports_cancel_and_finish() {
        let session = std::sync::Arc::new(SecureSession::new());
        assert_eq!(session.status().unwrap(), SessionStatus::Locked);
        let _guard = session.begin_initialization().unwrap();
        assert_eq!(session.status().unwrap(), SessionStatus::Initializing);
        assert!(session.begin_initialization().is_err());
        assert!(session.lock_and_wait().is_err());
        assert!(session
            .unlock_with_connection(rusqlite::Connection::open_in_memory().unwrap())
            .is_err());
        session.cancel_initialization().unwrap();
        assert_eq!(session.status().unwrap(), SessionStatus::Locked);
        let _second_guard = session.begin_initialization().unwrap();
        session
            .finish_initialization(rusqlite::Connection::open_in_memory().unwrap())
            .unwrap();
        assert_eq!(session.status().unwrap(), SessionStatus::Unlocked);
        assert!(session.begin_initialization().is_err());
        session.lock_and_wait().unwrap();
        assert_eq!(session.status().unwrap(), SessionStatus::Locked);
    }
}

#[cfg(test)]
mod password_change_session_tests {
    use super::{SecureSession, SessionStatus};
    use rusqlite::Connection;
    use std::sync::{mpsc, Arc};
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn password_change_reserves_unlocked_session_and_drop_locks_it() {
        let session = Arc::new(SecureSession::new());
        session
            .unlock_with_connection(Connection::open_in_memory().unwrap())
            .unwrap();

        let guard = session.begin_password_change().unwrap();

        assert_eq!(session.status().unwrap(), SessionStatus::Initializing);
        assert!(session.with_connection(|_| Ok(())).is_err());
        assert!(session.begin_password_change().is_err());

        // Незавершённая операция не должна оставлять старое соединение доступным.
        drop(guard);
        assert_eq!(session.status().unwrap(), SessionStatus::Locked);
        assert!(session.with_connection(|_| Ok(())).is_err());
    }

    #[test]
    fn password_change_waits_for_active_operations_and_can_finish() {
        let session = Arc::new(SecureSession::new());
        session
            .unlock_with_connection(Connection::open_in_memory().unwrap())
            .unwrap();

        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let worker_session = Arc::clone(&session);
        let worker = thread::spawn(move || {
            worker_session
                .with_connection(|_| {
                    started_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Ok(())
                })
                .unwrap();
        });

        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        let changing_session = Arc::clone(&session);
        let changer = thread::spawn(move || changing_session.begin_password_change().unwrap());

        let started_at = Instant::now();
        while session.status().unwrap() != SessionStatus::Locking {
            assert!(
                started_at.elapsed() < Duration::from_secs(5),
                "Сессия не перешла в Locking"
            );
            thread::yield_now();
        }

        assert!(session.with_connection(|_| Ok(())).is_err());

        release_tx.send(()).unwrap();
        worker.join().unwrap();

        let guard = changer.join().unwrap();
        assert_eq!(session.status().unwrap(), SessionStatus::Initializing);

        // Имитируем повторное открытие базы после смены ключа.
        guard.finish(Connection::open_in_memory().unwrap()).unwrap();

        assert_eq!(session.status().unwrap(), SessionStatus::Unlocked);
        session
            .with_connection(|conn| {
                conn.query_row("SELECT 42", [], |row| row.get::<_, i64>(0))
                    .map(|value| {
                        assert_eq!(value, 42);
                    })
                    .map_err(|error| error.to_string())
            })
            .unwrap();
    }

    #[test]
    fn password_change_can_restore_session_with_old_connection() {
        let session = Arc::new(SecureSession::new());
        session
            .unlock_with_connection(Connection::open_in_memory().unwrap())
            .unwrap();

        let guard = session.begin_password_change().unwrap();

        // Имитируем ошибку смены пароля и успешное повторное открытие
        // исходной базы со старым паролем.
        guard.finish(Connection::open_in_memory().unwrap()).unwrap();

        assert_eq!(session.status().unwrap(), SessionStatus::Unlocked);
        assert!(session.with_connection(|_| Ok(())).is_ok());
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "secure_session/waiting_operation_test.rs"]
mod waiting_operation_test;

#[cfg(test)]
#[path = "secure_session/entry_integration_test.rs"]
mod entry_integration_test;

#[cfg(test)]
#[path = "secure_session/transaction_test.rs"]
mod transaction_test;

#[cfg(test)]
#[path = "secure_session/backup_integration_test.rs"]
mod backup_integration_test;

#[cfg(test)]
#[path = "secure_session/reset_integration_test.rs"]
mod reset_integration_test;
#[cfg(test)]
#[path = "secure_session/sqlcipher_integration_test.rs"]
mod sqlcipher_integration_test;
