
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;

#[derive(Default)]
struct BarrierState {
    migration_requested: bool,
    active_writes: usize,
    failed_writes: usize,
}

struct WriteBarrier {
    state: Mutex<BarrierState>,
    changed: Condvar,
}

impl WriteBarrier {
    fn new() -> Self {
        Self {
            state: Mutex::new(BarrierState::default()),
            changed: Condvar::new(),
        }
    }

    fn begin_write(self: &Arc<Self>) -> Option<WritePermit> {
        let mut state = self.state.lock().unwrap();

        if state.migration_requested {
            return None;
        }

        state.active_writes += 1;

        Some(WritePermit {
            barrier: Arc::clone(self),
            succeeded: false,
        })
    }

    fn request_migration(&self) {
        let mut state = self.state.lock().unwrap();
        state.migration_requested = true;
        self.changed.notify_all();
    }

    fn wait_until_safe(&self) -> bool {
        let mut state = self.state.lock().unwrap();

        while state.active_writes > 0 {
            state = self.changed.wait(state).unwrap();
        }

        state.failed_writes == 0
    }
}

struct WritePermit {
    barrier: Arc<WriteBarrier>,
    succeeded: bool,
}

impl Drop for WritePermit {
    fn drop(&mut self) {
        let mut state = self.barrier.state.lock().unwrap();

        state.active_writes -= 1;

        if !self.succeeded {
            state.failed_writes += 1;
        }

        self.barrier.changed.notify_all();
    }
}

fn main() {
    println!("Начинаем тест аварийного завершения потока.");

    let barrier = Arc::new(WriteBarrier::new());

    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (continue_tx, continue_rx) = mpsc::channel::<()>();

    let writer_barrier = Arc::clone(&barrier);

    let writer = thread::spawn(move || {
        let _permit = writer_barrier
            .begin_write()
            .expect("Запись должна быть разрешена");

        println!("1. Поток начал операцию записи.");

        started_tx
            .send(())
            .expect("Не удалось подтвердить начало записи");

        continue_rx
            .recv()
            .expect("Не получено разрешение продолжить");

        // Имитируем панику во время сохранения.
        panic!("Искусственная ошибка потока записи");
    });

    // Дожидаемся начала операции записи.
    started_rx
        .recv()
        .expect("Не получено подтверждение начала записи");

    // Запрещаем новые записи.
    barrier.request_migration();

    println!("2. Миграция запрошена, новые записи заблокированы.");

    assert!(barrier.begin_write().is_none());

    // Разрешаем потоку продолжить и вызвать panic!.
    continue_tx
        .send(())
        .expect("Не удалось разрешить продолжение");

    // Дожидаемся завершения потока.
    let writer_result = writer.join();

    assert!(writer_result.is_err());

    println!("3. Подтверждено аварийное завершение потока.");

    // Drop должен был освободить счётчик активных записей
    // и отметить операцию как неуспешную.
    let state = barrier.state.lock().unwrap();

    assert_eq!(state.active_writes, 0);
    assert_eq!(state.failed_writes, 1);

    drop(state);

    println!("4. WritePermit освободил счётчик активных записей.");
    println!("5. Неудачная запись зарегистрирована.");

    // Миграция не должна быть разрешена.
    let migration_allowed = barrier.wait_until_safe();

    assert!(!migration_allowed);

    println!("6. Миграция запрещена.");
    println!("Тест аварийного завершения потока успешно пройден!");
}
