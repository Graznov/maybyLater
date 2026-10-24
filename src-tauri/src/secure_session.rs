
use rusqlite::Connection;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Locked,
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
        operation: impl FnOnce(&Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let _permit = self.begin_operation()?;

        let connection = self.connection.lock().map_err(|e| e.to_string())?;

        // Повторная проверка после ожидания SQLite Mutex.
        let state = self.state.lock().map_err(|e| e.to_string())?;

        if state.status != SessionStatus::Unlocked {
            return Err("Операция отменена: дневник блокируется".into());
        }

        let conn = connection.as_ref().ok_or("SQLite-соединение отсутствует")?;

        drop(state);

        operation(conn)
    }

    pub fn lock_and_wait(&self) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;

        match state.status {
            SessionStatus::Locked => return Ok(()),
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
mod tests;

#[cfg(test)]
#[path = "secure_session/waiting_operation_test.rs"]
mod waiting_operation_test;

#[cfg(test)]
#[path = "secure_session/entry_integration_test.rs"]
mod entry_integration_test;