
use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

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

    fn begin_migration(&self) {
        let mut state = self.state.lock().unwrap();

        state.migration_requested = true;

        while state.active_writes > 0 {
            state = self.changed.wait(state).unwrap();
        }
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
    let source_path = "barrier-source.db";
    let encrypted_path = "barrier-encrypted.db";
    let password = "laboratory-barrier-password";

    for path in [
        source_path,
        "barrier-source.db-wal",
        "barrier-source.db-shm",
        encrypted_path,
        "barrier-encrypted.db-wal",
        "barrier-encrypted.db-shm",
    ] {
        if Path::new(path).exists() {
            println!("Файл {path} уже существует. Ничего не изменено.");
            return Ok(());
        }
    }

    // 1. Создаём тестовую базу.
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

    // 2. Запускаем запись в отдельном потоке.
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

        thread::sleep(Duration::from_millis(500));

        conn.execute_batch("COMMIT;")?;

        println!("2. Текущая запись завершена.");

        Ok(())
    });

    // Даём потоку время начать запись.
    thread::sleep(Duration::from_millis(100));

    // 3. Запрещаем новые записи и ждём текущую.
    barrier.begin_migration();

    println!("3. Барьер дождался завершения записи.");

    writer.join().expect("Ошибка потока записи")?;

    // 4. Проверяем, что новая запись запрещена.
    assert!(barrier.begin_write().is_none());

    println!("4. Новые записи заблокированы.");

    // 5. Выполняем экспорт только после завершения записи.
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

    println!("5. Экспорт в SQLCipher завершён.");

    // 6. Проверяем зашифрованную базу.
    let encrypted = Connection::open(encrypted_path)?;
    encrypted.pragma_update(None, "key", password)?;

    let count: i64 =
        encrypted.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))?;

    assert_eq!(count, 2);

    println!("6. Обе записи присутствуют в зашифрованной базе.");

    println!("Тест барьера записи завершён успешно!");

    Ok(())
}
