
use rusqlite::{Connection, Result};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MigrationState {
    Idle,
    Requested,
    Exporting,
    RecoveryRequired,
}

struct BarrierState {
    migration: MigrationState,
    active_writes: usize,
    failed_writes: usize,
}

struct WriteBarrier {
    state: Mutex<BarrierState>,
}

impl WriteBarrier {
    fn new() -> Self {
        Self {
            state: Mutex::new(BarrierState {
                migration: MigrationState::Idle,
                active_writes: 0,
                failed_writes: 0,
            }),
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
        true
    }

    fn report_export_failure(&self) -> bool {
        let mut state = self.state.lock().unwrap();

        if state.migration != MigrationState::Exporting {
            return false;
        }

        state.migration = MigrationState::RecoveryRequired;
        true
    }

    fn complete_recovery(&self) -> bool {
        let mut state = self.state.lock().unwrap();

        if state.migration != MigrationState::RecoveryRequired
            || state.active_writes != 0
            || state.failed_writes != 0
        {
            return false;
        }

        state.migration = MigrationState::Idle;
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
    }
}

fn main() -> Result<()> {
    let source_path = "failed-migration-source.db";
    let partial_path = "failed-migration-partial.db";

    // Никогда не перезаписываем файлы предыдущего запуска.
    for path in [
        source_path,
        "failed-migration-source.db-wal",
        "failed-migration-source.db-shm",
        "failed-migration-source.db-journal",
        partial_path,
        "failed-migration-partial.db-wal",
        "failed-migration-partial.db-shm",
        "failed-migration-partial.db-journal",
    ] {
        if Path::new(path).exists() {
            println!("Файл {path} уже существует. Ничего не изменено.");
            return Ok(());
        }
    }

    // 1. Создаём искусственную исходную базу.
    let source = Connection::open(source_path)?;

    source.execute_batch(
        "
        CREATE TABLE entries (
            id TEXT PRIMARY KEY NOT NULL,
            content TEXT NOT NULL
        );

        INSERT INTO entries (id, content)
        VALUES ('entry-1', 'Исходная запись');
        ",
    )?;

    drop(source);

    println!("1. Искусственная база SQLite создана.");

    let barrier = Arc::new(WriteBarrier::new());

    // 2. Имитируем успешное сохранение перед миграцией.
    let mut permit = barrier
        .begin_write()
        .expect("Обычная запись должна быть разрешена");

    permit.mark_success();
    drop(permit);

    assert!(barrier.request_migration());
    assert!(barrier.begin_write().is_none());

    println!("2. Миграция запрошена, новые записи заблокированы.");

    // 3. Переходим в состояние экспорта.
    assert!(barrier.begin_export());

    println!("3. Фаза экспорта началась.");

    // 4. Создаём искусственную неполную копию.
    // Это НЕ настоящий SQLCipher-экспорт.
    let partial = Connection::open(partial_path)?;

    partial.execute_batch(
        "
        CREATE TABLE incomplete_export (
            id INTEGER PRIMARY KEY
        );
        ",
    )?;

    drop(partial);

    println!("4. Создана искусственная неполная копия.");

    // 5. Сообщаем барьеру об ошибке экспорта.
    assert!(barrier.report_export_failure());

    assert!(barrier.begin_write().is_none());

    println!("5. Ошибка зарегистрирована, записи остаются заблокированными.");

    // 6. Проверяем исходную базу только на чтение.
    let original = Connection::open_with_flags(
        source_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;

    let content: String = original.query_row(
        "SELECT content FROM entries WHERE id = 'entry-1'",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(content, "Исходная запись");

    let count: i64 =
        original.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))?;

    assert_eq!(count, 1);

    drop(original);

    println!("6. Исходная база проверена: данные не изменились.");

    // 7. Проверяем, что неполная копия не стала рабочей базой.
    assert!(Path::new(partial_path).exists());

    let state = barrier.state.lock().unwrap();

    assert_eq!(state.migration, MigrationState::RecoveryRequired);

    drop(state);

    println!("7. Неполная копия изолирована от исходной базы.");

    // 8. Подтверждаем безопасное восстановление.
    // В этом лабораторном тесте подтверждение ручное:
    // мы проверили исходную базу и не переключали пути.
    assert!(barrier.complete_recovery());

    println!("8. Восстановление подтверждено.");

    // 9. Проверяем возобновление записей.
    let mut new_permit = barrier
        .begin_write()
        .expect("После восстановления запись должна быть разрешена");

    new_permit.mark_success();
    drop(new_permit);

    println!("9. После восстановления записи снова разрешены.");

    let state = barrier.state.lock().unwrap();

    assert_eq!(state.migration, MigrationState::Idle);
    assert_eq!(state.active_writes, 0);
    assert_eq!(state.failed_writes, 0);

    println!("10. Итоговое состояние барьера проверено.");
    println!("Тест восстановления после ошибки миграции успешно пройден!");

    Ok(())
}
