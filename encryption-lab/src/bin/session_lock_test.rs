use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionStatus {
    Locked,
    Unlocked,
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

    fn unlock(&self) {
        let mut state = self.state.lock().unwrap();
        state.status = SessionStatus::Unlocked;
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

        // Сначала запрещаем новые операции чтения и записи.
        state.status = SessionStatus::Locked;

        // Затем ждём завершения уже начатых операций.
        while state.active_operations > 0 {
            state = self.changed.wait(state).unwrap();
        }
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

    // 1. При запуске дневник заблокирован.
    assert_eq!(session.status(), SessionStatus::Locked);
    assert!(session.begin_operation().is_none());
    println!("1. Заблокированная сессия запрещает доступ.");

    // 2. Имитируем успешную разблокировку.
    // Здесь нет настоящего пароля или ключа SQLCipher.
    session.unlock();

    assert_eq!(session.status(), SessionStatus::Unlocked);
    println!("2. Сессия разблокирована.");

    // 3. Запускаем операцию чтения или записи.
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (finish_tx, finish_rx) = mpsc::channel::<()>();

    let worker_session = Arc::clone(&session);

    let worker = thread::spawn(move || {
        let _permit = worker_session
            .begin_operation()
            .expect("Операция должна быть разрешена");

        started_tx.send(()).unwrap();
        finish_rx.recv().unwrap();

        println!("3. Начатая операция завершена.");
    });

    started_rx.recv().unwrap();

    // 4. Запрашиваем блокировку из другого потока.
    let locker_session = Arc::clone(&session);
    let (locked_tx, locked_rx) = mpsc::channel::<()>();

    let locker = thread::spawn(move || {
        locker_session.lock_and_wait();
        locked_tx.send(()).unwrap();
    });

    // Дожидаемся фактического перехода в Locked.
    // Не используем sleep для синхронизации.
    {
        let mut state = session.state.lock().unwrap();

        while state.status != SessionStatus::Locked {
            drop(state);
            thread::yield_now();
            state = session.state.lock().unwrap();
        }

        assert_eq!(state.active_operations, 1);
    }

    // 5. Новые операции уже запрещены.
    assert!(session.begin_operation().is_none());

    // Блокировка ещё ждёт завершения старой операции.
    assert!(locked_rx.try_recv().is_err());

    println!("4. Новые операции запрещены, старая ещё выполняется.");

    // 6. Разрешаем старой операции завершиться.
    finish_tx.send(()).unwrap();

    worker.join().unwrap();
    locker.join().unwrap();
    locked_rx.recv().unwrap();

    assert_eq!(session.status(), SessionStatus::Locked);
    assert!(session.begin_operation().is_none());

    println!("5. Блокировка дождалась завершения операции.");
    println!("6. После блокировки доступ остаётся запрещён.");

    // 7. Проверяем повторную разблокировку.
    session.unlock();

    let permit = session
        .begin_operation()
        .expect("После разблокировки доступ должен восстановиться");

    drop(permit);

    session.lock_and_wait();

    assert_eq!(session.status(), SessionStatus::Locked);
    assert!(session.begin_operation().is_none());

    println!("7. Повторная разблокировка и блокировка работают.");
    println!("Тест блокировки сессии успешно пройден!");
}