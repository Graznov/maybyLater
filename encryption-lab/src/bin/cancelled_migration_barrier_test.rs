
use std::sync::{Arc, Condvar, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MigrationState {
    Idle,
    Requested,
    Exporting,
}

struct BarrierState {
    migration: MigrationState,
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
            state: Mutex::new(BarrierState {
                migration: MigrationState::Idle,
                active_writes: 0,
                failed_writes: 0,
            }),
            changed: Condvar::new(),
        }
    }

    fn begin_write(self: &Arc<Self>) -> Option<WritePermit> {
        let mut state = self.state.lock().unwrap();

        if state.migration != MigrationState::Idle {
            return None;
        }

        state.active_writes += 1;

        Some(WritePermit {
            barrier: Arc::clone(self),
            succeeded: false,
        })
    }

    fn request_migration(&self) -> bool {
        let mut state = self.state.lock().unwrap();

        if state.migration != MigrationState::Idle {
            return false;
        }

        state.migration = MigrationState::Requested;
        self.changed.notify_all();

        true
    }

    fn cancel_migration(&self) -> bool {
        let mut state = self.state.lock().unwrap();

        // Отменять разрешено только до начала экспорта.
        if state.migration != MigrationState::Requested {
            return false;
        }

        state.migration = MigrationState::Idle;
        self.changed.notify_all();

        true
    }

    fn begin_export(&self) -> bool {
        let mut state = self.state.lock().unwrap();

        if state.migration != MigrationState::Requested
            || state.active_writes != 0
            || state.failed_writes != 0
        {
            return false;
        }

        state.migration = MigrationState::Exporting;
        self.changed.notify_all();

        true
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
    println!("Начинаем тест отмены миграции.");

    let barrier = Arc::new(WriteBarrier::new());

    // 1. Обычная запись разрешена.
    let mut first_write = barrier
        .begin_write()
        .expect("Первая запись должна быть разрешена");

    first_write.mark_success();
    drop(first_write);

    println!("1. Обычная запись успешно завершена.");

    // 2. Запрашиваем миграцию.
    assert!(barrier.request_migration());

    println!("2. Миграция запрошена.");

    // 3. Новые записи запрещены.
    assert!(barrier.begin_write().is_none());

    println!("3. Новые записи заблокированы.");

    // 4. Отменяем миграцию до начала экспорта.
    assert!(barrier.cancel_migration());

    println!("4. Миграция безопасно отменена.");

    // 5. После отмены запись снова разрешена.
    let mut second_write = barrier
        .begin_write()
        .expect("После отмены запись должна быть разрешена");

    second_write.mark_success();
    drop(second_write);

    println!("5. Записи снова разрешены.");

    // 6. Повторно запрашиваем миграцию.
    assert!(barrier.request_migration());

    println!("6. Повторная миграция запрошена.");

    // 7. Начинаем лабораторную фазу экспорта.
    assert!(barrier.begin_export());

    println!("7. Фаза экспорта началась.");

    // 8. После начала экспорта отмена запрещена.
    assert!(!barrier.cancel_migration());

    println!("8. Отмена во время экспорта отклонена.");

    // 9. Записи по-прежнему заблокированы.
    assert!(barrier.begin_write().is_none());

    let state = barrier.state.lock().unwrap();

    assert_eq!(state.migration, MigrationState::Exporting);
    assert_eq!(state.active_writes, 0);
    assert_eq!(state.failed_writes, 0);

    println!("9. Состояние барьера проверено.");
    println!("Тест отмены миграции успешно пройден!");
}
