
use rusqlite::{Connection, Result};
use std::path::Path;
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
    #[allow(dead_code)]
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

fn main() -> Result<()> {
    let source_path = "failed-write-source.db";
    let encrypted_path = "failed-write-encrypted.db";

    // Не изменяем файлы предыдущих запусков.
    for path in [
        source_path,
        "failed-write-source.db-wal",
        "failed-write-source.db-shm",
        encrypted_path,
    ] {
        if Path::new(path).exists() {
            println!("Файл {path} уже существует. Ничего не изменено.");
            return Ok(());
        }
    }

    // 1. Создаём искусственную базу.
    let setup = Connection::open(source_path)?;

    setup.execute_batch(
        "
        CREATE TABLE entries (
            id TEXT PRIMARY KEY NOT NULL,
            content TEXT NOT NULL
        );

        INSERT INTO entries (id, content)
        VALUES ('entry-1', 'Первая запись');
        ",
    )?;

    drop(setup);

    println!("1. Тестовая база создана.");

    let barrier = Arc::new(WriteBarrier::new());

    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (finish_tx, finish_rx) = mpsc::channel::<()>();

    let writer_barrier = Arc::clone(&barrier);
    let writer_path = source_path.to_string();

    let writer = thread::spawn(move || -> Result<()> {
        let permit = writer_barrier
            .begin_write()
            .expect("Запись должна быть разрешена");

        let conn = Connection::open(&writer_path)?;

        // Намеренно нарушаем PRIMARY KEY.
        let failed_result = conn.execute(
            "INSERT INTO entries (id, content)
             VALUES ('entry-1', 'Дубликат записи')",
            [],
        );

        assert!(failed_result.is_err());

        println!("2. SQLite отклонила дублирующую запись.");

        started_tx.send(()).expect("Ошибка подтверждения записи");
        finish_rx.recv().expect("Нет разрешения завершить операцию");

        // mark_success() НЕ вызываем:
        // сохранение завершилось ошибкой.
        drop(permit);

        Ok(())
    });

    // 2. Дожидаемся подтверждения ошибки.
    started_rx.recv().expect("Не получено подтверждение");

    // 3. Запрашиваем миграцию.
    barrier.request_migration();

    println!("3. Миграция запрошена.");

    // 4. Разрешаем потоку завершить работу.
    finish_tx.send(()).expect("Ошибка разрешения завершения");

    let migration_allowed = barrier.wait_until_safe();

    writer.join().expect("Ошибка потока")?;

    // 5. Барьер обязан запретить миграцию.
    assert!(!migration_allowed);
    assert!(!Path::new(encrypted_path).exists());

    println!("4. Барьер обнаружил неудачное сохранение.");
    println!("5. Миграция запрещена, зашифрованная копия не создавалась.");

    // 6. Исходная запись должна сохраниться.
    let original = Connection::open(source_path)?;

    let content: String = original.query_row(
        "SELECT content FROM entries WHERE id = 'entry-1'",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(content, "Первая запись");

    println!("6. Исходная запись осталась без изменений.");
    println!("Тест обработки ошибки сохранения завершён успешно!");

    Ok(())
}
