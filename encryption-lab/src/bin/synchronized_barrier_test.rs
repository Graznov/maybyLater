
use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;

#[derive(Default)]
struct BarrierState {
    migration_requested: bool,
    active_writes: usize,
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
        })
    }

    fn request_migration(&self) {
        let mut state = self.state.lock().unwrap();
        state.migration_requested = true;
        self.changed.notify_all();
    }

    fn wait_for_writes(&self) {
        let mut state = self.state.lock().unwrap();

        while state.active_writes > 0 {
            state = self.changed.wait(state).unwrap();
        }
    }

    fn is_migration_requested(&self) -> bool {
        self.state.lock().unwrap().migration_requested
    }
}

struct WritePermit {
    barrier: Arc<WriteBarrier>,
}

impl Drop for WritePermit {
    fn drop(&mut self) {
        let mut state = self.barrier.state.lock().unwrap();
        state.active_writes -= 1;
        self.barrier.changed.notify_all();
    }
}

fn main() -> Result<()> {
    let source_path = "synchronized-source.db";
    let encrypted_path = "synchronized-encrypted.db";
    let password = "laboratory-synchronized-password";

    // Не перезаписываем результаты предыдущих запусков.
    for path in [
        source_path,
        "synchronized-source.db-wal",
        "synchronized-source.db-shm",
        encrypted_path,
        "synchronized-encrypted.db-wal",
        "synchronized-encrypted.db-shm",
    ] {
        if Path::new(path).exists() {
            println!("Файл {path} уже существует. Ничего не изменено.");
            return Ok(());
        }
    }

    // 1. Создаём исходную тестовую базу.
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

    // Канал: запись действительно началась.
    let (started_tx, started_rx) = mpsc::channel::<()>();

    // Канал: текущей записи разрешено завершиться.
    let (finish_tx, finish_rx) = mpsc::channel::<()>();

    let writer_barrier = Arc::clone(&barrier);
    let writer_path = source_path.to_string();

    let writer = thread::spawn(move || -> Result<()> {
        let _permit = writer_barrier
            .begin_write()
            .expect("Запись должна быть разрешена");

        let conn = Connection::open(&writer_path)?;
        conn.execute_batch("BEGIN IMMEDIATE;")?;

        conn.execute(
            "INSERT INTO entries (id, content) VALUES (?1, ?2)",
            params!["entry-2", "Запись перед миграцией"],
        )?;

        started_tx.send(()).expect("Не удалось подтвердить начало записи");

        // Ожидаем явного разрешения, без sleep.
        finish_rx.recv().expect("Не получено разрешение завершить запись");

        conn.execute_batch("COMMIT;")?;
        println!("2. Текущая запись завершена.");

        Ok(())
    });

    // 2. Ждём подтверждения, что транзакция уже началась.
    started_rx.recv().expect("Запись не началась");
    println!("3. Подтверждено начало транзакции записи.");

    // 3. Запрашиваем миграцию и запрещаем новые записи.
    barrier.request_migration();

    assert!(barrier.is_migration_requested());
    assert!(barrier.begin_write().is_none());

    println!("4. Новые записи заблокированы.");

    // 4. Разрешаем текущей транзакции завершиться.
    finish_tx.send(()).expect("Не удалось разрешить завершение записи");

    barrier.wait_for_writes();
    writer.join().expect("Ошибка потока записи")?;

    println!("5. Барьер дождался завершения текущей записи.");

    // 5. Экспортируем только после завершения всех записей.
    let source = Connection::open(source_path)?;

    source.execute(
        "ATTACH DATABASE ?1 AS encrypted KEY ?2",
        params![encrypted_path, password],
    )?;

    source.query_row(
        "SELECT sqlcipher_export('encrypted')",
        [],
        |_row| Ok(()),
    )?;

    source.execute_batch("DETACH DATABASE encrypted;")?;

    println!("6. Экспорт в SQLCipher завершён.");

    // 6. Проверяем зашифрованную базу.
    let encrypted = Connection::open(encrypted_path)?;
    encrypted.pragma_update(None, "key", password)?;

    let count: i64 =
        encrypted.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))?;

    assert_eq!(count, 2);

    println!("7. Обе записи присутствуют в зашифрованной базе.");

    println!("Тест синхронизированного барьера завершён успешно!");

    Ok(())
}
