
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

impl WritePermit {
    fn mark_success(&mut self) {
        self.succeeded = true;
    }
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
    println!("Начинаем тест поздней записи.");

    let barrier = Arc::new(WriteBarrier::new());

    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (finish_tx, finish_rx) = mpsc::channel::<()>();

    let writer_barrier = Arc::clone(&barrier);

    // Первая операция начинается до запроса миграции.
    let writer = thread::spawn(move || {
        let mut permit = writer_barrier
            .begin_write()
            .expect("Первая запись должна быть разрешена");

        println!("1. Первая запись началась.");

        started_tx
            .send(())
            .expect("Не удалось подтвердить начало записи");

        finish_rx
            .recv()
            .expect("Не получено разрешение завершить запись");

        // Имитируем успешное сохранение.
        permit.mark_success();

        println!("3. Первая запись успешно завершена.");
    });

    // Ждём, пока первая операция действительно начнётся.
    started_rx
        .recv()
        .expect("Не получено подтверждение начала записи");

    // Запрашиваем миграцию, пока первая запись активна.
    barrier.request_migration();

    println!("2. Миграция запрошена.");

    // Попытка новой записи должна быть отклонена.
    assert!(barrier.begin_write().is_none());

    println!("4. Новая запись после запроса миграции заблокирована.");

    // Разрешаем первой операции завершиться.
    finish_tx
        .send(())
        .expect("Не удалось завершить первую запись");

    writer.join().expect("Ошибка потока записи");

    // Барьер должен дождаться завершения первой операции.
    let migration_allowed = barrier.wait_until_safe();

    assert!(migration_allowed);

    println!("5. Барьер подтвердил завершение первой записи.");
    println!("6. Миграция разрешена.");

    // Даже после завершения первой записи новые записи
    // должны оставаться заблокированными.
    assert!(barrier.begin_write().is_none());

    println!("7. Поздняя запись по-прежнему заблокирована.");

    // Проверяем итоговое состояние барьера.
    let state = barrier.state.lock().unwrap();

    assert_eq!(state.active_writes, 0);
    assert_eq!(state.failed_writes, 0);
    assert!(state.migration_requested);

    println!("8. Состояние барьера проверено.");
    println!("Тест поздней записи успешно пройден!");
}
