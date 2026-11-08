use rusqlite::Connection;
use std::path::Path;

const MARKER_CONTENT: &str = "maybe-later-encryption-v1\n";

fn prepare_source_for_switch(path: &Path) -> Result<(), String> {
    let conn = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    ).map_err(|e| format!("Не удалось подготовить SQLite к переключению: {e}"))?;

    let (busy, _, _): (i64, i64, i64) = conn
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(|e| format!("Не удалось завершить WAL: {e}"))?;

    if busy != 0 {
        return Err("Не удалось завершить WAL: остались активные читатели или писатели".into());
    }

    let mode: String = conn
        .query_row("PRAGMA journal_mode=DELETE", [], |row| row.get(0))
        .map_err(|e| format!("Не удалось переключить SQLite в режим DELETE: {e}"))?;

    if !mode.eq_ignore_ascii_case("delete") {
        return Err(format!("SQLite не перешла в режим DELETE: {mode}"));
    }

    drop(conn);

    let mut wal = path.as_os_str().to_os_string();
    wal.push("-wal");
    let mut shm = path.as_os_str().to_os_string();
    shm.push("-shm");

    if Path::new(&wal).exists() || Path::new(&shm).exists() {
        return Err("После закрытия SQLite остались файлы WAL/SHM. Переключение отменено".into());
    }

    Ok(())
}

fn ensure_marker(marker: &Path) -> Result<(), String> {
    if marker.exists() {
        let content = std::fs::read_to_string(marker)
            .map_err(|e| format!("Не удалось прочитать маркер миграции: {e}"))?;

        if content != MARKER_CONTENT {
            return Err("Маркер миграции имеет неизвестный формат".into());
        }

        return Ok(());
    }

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(marker)
        .map_err(|e| format!("Не удалось создать маркер миграции: {e}"))?;

    use std::io::Write;
    file.write_all(MARKER_CONTENT.as_bytes())
        .map_err(|e| format!("Не удалось записать маркер миграции: {e}"))?;

    file.sync_all()
        .map_err(|e| format!("Не удалось сохранить маркер миграции: {e}"))?;

    Ok(())
}

fn open_and_migrate(path: &Path, password: &str) -> Result<Connection, String> {
    let mut conn =
        crate::sqlcipher_connection::open_verified_database(path, password)?;

    crate::secure_migrations::run_pending_migrations(&mut conn)?;

    Ok(conn)
}

/// Безопасно переводит незашифрованную SQLite-базу в SQLCipher.
///
/// Исходная база сохраняется в backup_path.
/// При аварийном завершении состояние определяется по маркеру и файлам.
pub fn migrate(
    source: &Path,
    backup: &Path,
    encrypted: &Path,
    marker: &Path,
    password: &str,
) -> Result<Connection, String> {
    if password.is_empty() {
        return Err("Пароль не может быть пустым".into());
    }

    let parent = source.parent().ok_or("Не найден каталог базы")?;

    for path in [backup, encrypted, marker] {
        if path.parent() != Some(parent) {
            return Err("Все файлы миграции должны находиться в одном каталоге".into());
        }
    }

    if !marker.exists() && (backup.exists() || encrypted.exists()) {
        return Err(
            "Обнаружены файлы предыдущей миграции без маркера. Автоматическое переключение запрещено".into()
        );
    }

    ensure_marker(marker)?;

    // Восстановление после ситуации:
    // исходная база уже переименована, но новая ещё не опубликована.
    if !source.exists() && backup.exists() && !encrypted.exists() {
        std::fs::rename(backup, source)
            .map_err(|e| format!("Не удалось восстановить исходную SQLite-базу: {e}"))?;
    }

    if source.exists() {
        let plaintext =
            crate::sqlcipher_connection::has_plaintext_sqlite_header(source)?;

        if !plaintext {
            // Активная база уже зашифрована.
            if !backup.is_file() || encrypted.exists() {
                return Err("Неоднозначное состояние файлов миграции. Автоматическое восстановление остановлено".into());
            }

            let conn = open_and_migrate(source, password)?;

            std::fs::remove_file(marker)
                .map_err(|e| format!("База восстановлена, но не удалось удалить маркер: {e}"))?;

            return Ok(conn);
        }

        if backup.exists() {
            return Err("Резервная копия уже существует, но активная база осталась незашифрованной. Требуется проверка файлов".into());
        }

        // Если экспорт ещё не выполнялся, создаём SQLCipher-копию.
        if !encrypted.exists() {
            crate::sqlcipher_connection::export_plaintext_to_encrypted(
                source,
                encrypted,
                password,
            )?;
        }

        // Проверяем SQLCipher и применяем миграции до переключения.
        let conn = open_and_migrate(encrypted, password)?;
        drop(conn);

        // Завершаем WAL и закрываем боковые файлы перед переименованием.
        prepare_source_for_switch(source)?;

        // Сохраняем оригинал. Никогда не удаляем его.
        std::fs::rename(source, backup)
            .map_err(|e| format!("Не удалось сохранить исходную SQLite-базу: {e}"))?;

        // Публикуем зашифрованную базу.
        if let Err(error) = std::fs::rename(encrypted, source) {
            let rollback = std::fs::rename(backup, source);

            return match rollback {
                Ok(()) => Err(format!(
                    "Не удалось переключить базу: {error}. Исходная SQLite восстановлена"
                )),
                Err(rollback_error) => Err(format!(
                    "Не удалось переключить базу: {error}. Не удалось автоматически вернуть оригинал: {rollback_error}. Маркер сохранён"
                )),
            };
        }
    } else if backup.exists() && encrypted.exists() {
        // Восстановление после аварии между двумя переименованиями.
        let conn = open_and_migrate(encrypted, password)?;
        drop(conn);

        std::fs::rename(encrypted, source)
            .map_err(|e| format!("Не удалось завершить переключение базы: {e}"))?;
    } else {
        return Err("Не удалось определить безопасное состояние файлов миграции".into());
    }

    // Последняя проверка уже на активном пути.
    let conn = open_and_migrate(source, password)?;

    // Маркер удаляется только после успешной проверки активной базы.
    std::fs::remove_file(marker)
        .map_err(|e| format!("База переключена, но маркер миграции не удалён: {e}"))?;

    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::{migrate, MARKER_CONTENT};
    use crate::sqlcipher_connection::{has_plaintext_sqlite_header, open_verified_database};
    use rusqlite::Connection;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    const TEST_PASSWORD: &str = "test-only-migration-password";

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Системное время должно быть позже UNIX epoch")
                .as_nanos();

            let path = std::env::temp_dir().join(format!(
                "maybe-later-plaintext-migration-{}-{unique}",
                std::process::id()
            ));

            std::fs::create_dir(&path).expect("Не удалось создать каталог теста");
            Self(path)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn create_legacy_database(path: &Path) {
        let conn = Connection::open(path).expect("Не удалось создать исходную SQLite-базу");

        conn.execute_batch(
            "
            CREATE TABLE entries (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                date TEXT NOT NULL,
                tags TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO entries
                (id, title, content, date, tags, created_at, updated_at)
            VALUES
                ('entry-1', 'Старая запись', 'Содержимое записи', '2026-01-15',
                 '[\"важное\"]', '2026-01-15T10:00:00Z', '2026-01-15T10:00:00Z');
            ",
        )
        .expect("Не удалось заполнить исходную SQLite-базу");

        drop(conn);
    }

    fn migration_paths(directory: &TestDirectory) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        (
            directory.path("maybe-later.db"),
            directory.path("maybe-later-plaintext-backup.db"),
            directory.path("maybe-later-encrypted-pending.db"),
            directory.path("maybe-later-encryption-pending"),
        )
    }

    fn assert_entry_exists(conn: &Connection) {
        let (title, content): (String, String) = conn
            .query_row(
                "SELECT title, content FROM entries WHERE id = 'entry-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("Исходная запись должна сохраниться");

        assert_eq!(title, "Старая запись");
        assert_eq!(content, "Содержимое записи");
    }

    #[test]
    fn migrates_plaintext_database_and_keeps_original_backup() {
        let directory = TestDirectory::new();
        let (source, backup, encrypted, marker) = migration_paths(&directory);
        create_legacy_database(&source);

        assert!(has_plaintext_sqlite_header(&source).unwrap());

        let migrated = migrate(
            &source,
            &backup,
            &encrypted,
            &marker,
            TEST_PASSWORD,
        )
        .expect("Перенос SQLite в SQLCipher должен завершиться успешно");

        assert!(!has_plaintext_sqlite_header(&source).unwrap());
        assert!(backup.is_file(), "Исходная база должна остаться в резервной копии");
        assert!(!encrypted.exists(), "Временный зашифрованный файл должен быть опубликован");
        assert!(!marker.exists(), "Маркер удаляется после успешного завершения");

        assert_entry_exists(&migrated);
        drop(migrated);

        let backup_conn = Connection::open(&backup).unwrap();
        assert_entry_exists(&backup_conn);
        drop(backup_conn);

        let reopened = open_verified_database(&source, TEST_PASSWORD)
            .expect("Зашифрованная база должна открываться правильным паролем");
        assert_entry_exists(&reopened);
        drop(reopened);

        assert!(
            open_verified_database(&source, "wrong-test-password").is_err(),
            "Неправильный пароль должен быть отклонён"
        );
    }

    #[test]
    fn resumes_after_crash_between_the_two_file_renames() {
        let directory = TestDirectory::new();
        let (source, backup, encrypted, marker) = migration_paths(&directory);
        create_legacy_database(&source);

        // Имитируем состояние после первого rename:
        // оригинал уже сохранён, зашифрованный файл ещё не опубликован.
        std::fs::write(&marker, MARKER_CONTENT).unwrap();
        crate::sqlcipher_connection::export_plaintext_to_encrypted(
            &source,
            &encrypted,
            TEST_PASSWORD,
        )
        .unwrap();
        std::fs::rename(&source, &backup).unwrap();

        assert!(!source.exists());
        assert!(backup.exists());
        assert!(encrypted.exists());
        assert!(marker.exists());

        let migrated = migrate(
            &source,
            &backup,
            &encrypted,
            &marker,
            TEST_PASSWORD,
        )
        .expect("Миграция должна продолжиться после прерывания");

        assert!(source.is_file());
        assert!(backup.is_file());
        assert!(!encrypted.exists());
        assert!(!marker.exists());
        assert!(!has_plaintext_sqlite_header(&source).unwrap());
        assert_entry_exists(&migrated);
    }

    #[test]
    fn preserves_plaintext_source_when_encrypted_copy_cannot_be_opened() {
        let directory = TestDirectory::new();
        let (source, backup, encrypted, marker) = migration_paths(&directory);
        create_legacy_database(&source);

        // Имитируем продолжение ранее начатой миграции:
        // маркер уже создан, а временная зашифрованная копия существует.
        std::fs::write(&marker, MARKER_CONTENT).unwrap();

        // Создаём зашифрованную копию с паролем, отличным от пароля попытки.
        crate::sqlcipher_connection::export_plaintext_to_encrypted(
            &source,
            &encrypted,
            "different-test-password",
        )
        .unwrap();

        let error = migrate(
            &source,
            &backup,
            &encrypted,
            &marker,
            TEST_PASSWORD,
        )
        .expect_err("Неправильный пароль не должен переключать активную базу");

        assert!(!error.is_empty());
        assert!(source.is_file());
        assert!(backup.exists() == false);
        assert!(encrypted.is_file());
        assert!(marker.is_file());
        assert!(has_plaintext_sqlite_header(&source).unwrap());

        let original = Connection::open(&source).unwrap();
        assert_entry_exists(&original);
    }

    #[test]
    fn rejects_orphaned_migration_files_without_marker() {
        let directory = TestDirectory::new();
        let (source, backup, encrypted, marker) = migration_paths(&directory);
        create_legacy_database(&source);
        std::fs::write(&encrypted, b"orphaned temporary file").unwrap();

        let error = migrate(
            &source,
            &backup,
            &encrypted,
            &marker,
            TEST_PASSWORD,
        )
        .expect_err("Файлы без маркера не должны переключаться автоматически");

        assert!(error.contains("без маркера"));
        assert!(source.is_file());
        assert!(encrypted.is_file());
        assert!(!backup.exists());
        assert!(!marker.exists());

        let original = Connection::open(&source).unwrap();
        assert_entry_exists(&original);
    }
}
