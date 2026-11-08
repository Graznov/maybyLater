
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
        Ok(InitializationGuard { session: Arc::clone(self) })
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

        let conn = connection
            .as_mut()
            .ok_or("SQLite-соединение отсутствует")?;

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
        assert!(session.unlock_with_connection(rusqlite::Connection::open_in_memory().unwrap()).is_err());
        session.cancel_initialization().unwrap();
        assert_eq!(session.status().unwrap(), SessionStatus::Locked);
        let _second_guard = session.begin_initialization().unwrap();
        session.finish_initialization(rusqlite::Connection::open_in_memory().unwrap()).unwrap();
        assert_eq!(session.status().unwrap(), SessionStatus::Unlocked);
        assert!(session.begin_initialization().is_err());
        session.lock_and_wait().unwrap();
        assert_eq!(session.status().unwrap(), SessionStatus::Locked);
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
