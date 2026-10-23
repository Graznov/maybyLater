use std::sync::{mpsc, Arc, Condvar, Mutex};
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
}

struct SessionLock {
    state: Mutex<SessionState>,
    changed: Condvar,
}

impl SessionLock {
    fn new() -> Self {
        Self {
            state: Mutex::new(SessionState {
                status: SessionStatus::Locked,
                active_operations: 0,
            }),
            changed: Condvar::new(),
        }
    }

    fn unlock(&self) -> Result<(), &'static str> {
        let mut state = self.state.lock().unwrap();

        if state.status == SessionStatus::Locking {
            return Err("Разблокировка запрещена: блокировка ещё выполняется");
        }

        // Только имитация разблокировки.
        // Проверки пароля и ключа SQLCipher здесь нет.
        state.status = SessionStatus::Unlocked;

        Ok(())
    }

    fn begin_operation(self: &Arc<Self>) -> Option<OperationPermit> {
        let mut state = self.state.lock().unwrap();

        if state.status != SessionStatus::Unlocked {
            return None;
        }

        state.active_operations += 1;

        Some(OperationPermit {
            session: Arc::clone(self),
        })
    }

    fn lock_and_wait(&self) {
        let mut state = self.state.lock().unwrap();

        // Запрещаем новые операции до ожидания старых.
        state.status = SessionStatus::Locking;
        self.changed.notify_all();

        while state.active_operations > 0 {
            state = self.changed.wait(state).unwrap();
        }

        state.status = SessionStatus::Locked;
        self.changed.notify_all();
    }

    fn status(&self) -> SessionStatus {
        self.state.lock().unwrap().status
    }
}

struct OperationPermit {
    session: Arc<SessionLock>,
}

impl Drop for OperationPermit {
    fn drop(&mut self) {
        let mut state = self.session.state.lock().unwrap();

        state.active_operations -= 1;
        self.session.changed.notify_all();
    }
}

fn main() {
    let session = Arc::new(SessionLock::new());

    assert_eq!(session.status(), SessionStatus::Locked);
    assert!(session.begin_operation().is_none());

    println!("1. Начальное состояние Locked подтверждено.");

    session.unlock().expect("Первая разблокировка должна пройти");

    assert_eq!(session.status(), SessionStatus::Unlocked);

    println!("2. Сессия разблокирована.");

    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (finish_tx, finish_rx) = mpsc::channel::<()>();

    let worker_session = Arc::clone(&session);

    let worker = thread::spawn(move || {
        let _permit = worker_session
            .begin_operation()
            .expect("Операция должна быть разрешена");

        started_tx.send(()).unwrap();

        // Удерживаем разрешение, пока основной поток
        // не разрешит завершение операции.
        finish_rx.recv().unwrap();
    });

    started_rx.recv().unwrap();

    println!("3. Операция началась и удерживает разрешение.");

    let locker_session = Arc::clone(&session);

    let (locking_tx, locking_rx) = mpsc::channel::<()>();

    let locker = thread::spawn(move || {
        locker_session.lock_and_wait();
        locking_tx.send(()).unwrap();
    });

    // Ожидаем фактического перехода в Locking.
    // Здесь не используем sleep.
    {
        let mut state = session.state.lock().unwrap();

        while state.status != SessionStatus::Locking {
            state = session.changed.wait(state).unwrap();
        }

        assert_eq!(state.active_operations, 1);
    }

    println!("4. Сессия перешла в Locking.");

    // Главное испытание:
    // unlock не должен отменить выполняющуюся блокировку.
    let unlock_result = session.unlock();

    assert!(unlock_result.is_err());
    assert_eq!(session.status(), SessionStatus::Locking);
    assert!(session.begin_operation().is_none());

    println!("5. Разблокировка во время Locking отклонена.");

    // Блокировка ещё не должна завершиться.
    assert!(locking_rx.try_recv().is_err());

    finish_tx.send(()).unwrap();

    worker.join().unwrap();
    locker.join().unwrap();
    locking_rx.recv().unwrap();

    assert_eq!(session.status(), SessionStatus::Locked);
    assert!(session.begin_operation().is_none());

    println!("6. Старая операция завершена, сессия заблокирована.");

    // Теперь новая разблокировка допустима.
    session
        .unlock()
        .expect("После завершения блокировки разблокировка разрешена");

    assert_eq!(session.status(), SessionStatus::Unlocked);

    let permit = session
        .begin_operation()
        .expect("После разблокировки операция должна быть разрешена");

    drop(permit);
    session.lock_and_wait();

    assert_eq!(session.status(), SessionStatus::Locked);

    println!("7. Повторная разблокировка и блокировка прошли.");
    println!("Тест гонки unlock/lock успешно пройден!");
}