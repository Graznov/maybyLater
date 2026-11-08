use rusqlite::{Connection, OpenFlags};
use std::path::Path;

/// Определяет стандартный незашифрованный заголовок SQLite без открытия базы.
pub fn has_plaintext_sqlite_header(path: &Path) -> Result<bool, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("Не удалось прочитать заголовок дневника: {e}"))?;
    let mut header = [0u8; 16];
    file.read_exact(&mut header)
        .map_err(|e| format!("Не удалось прочитать заголовок дневника: {e}"))?;
    Ok(&header == b"SQLite format 3\0")
}

/// Открывает существующую SQLCipher-базу для чтения и записи.
/// Возвращает соединение лишь после проверки пароля и целостности.
pub fn open_verified_database(
    path: &Path,
    password: &str,
) -> Result<Connection, String> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE,
    )
    .map_err(|e| e.to_string())?;

    conn.pragma_update(None, "key", password)
        .map_err(|e| e.to_string())?;

    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|_| "Неверный пароль или повреждённая база".to_string())?;

    if result != "ok" {
        return Err(format!("Ошибка целостности базы: {result}"));
    }

    {
        let mut stmt = conn.prepare("PRAGMA cipher_integrity_check")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query([]).map_err(|e| e.to_string())?;

        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let error: String = row.get(0).map_err(|e| e.to_string())?;
            return Err(format!("Ошибка целостности SQLCipher: {error}"));
        }
    }

    Ok(conn)
}

fn list_user_tables(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type = ?1 AND name NOT LIKE ?2 ORDER BY name")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map(["table", "sqlite_%"], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

fn read_table_rows(conn: &Connection, table: &str) -> Result<Vec<Vec<rusqlite::types::Value>>, String> {
    let quoted = format!("\"{}\"", table.replace("\"", "\"\""));
    let mut stmt = conn.prepare(&format!("SELECT * FROM {quoted}"))
        .map_err(|e| format!("Не удалось прочитать таблицу {table}: {e}"))?;
    let count = stmt.column_count();
    let rows = stmt.query_map([], |row| {
        (0..count).map(|i| row.get::<_, rusqlite::types::Value>(i))
            .collect::<rusqlite::Result<Vec<_>>>()
    }).map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

fn read_schema_objects(conn: &Connection) -> Result<Vec<(String, String, String, Option<String>)>, String> {
    let mut stmt = conn.prepare("SELECT type, name, tbl_name, sql FROM sqlite_master WHERE name NOT LIKE ?1 ORDER BY type, name")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map(["sqlite_%"], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
}

/// Экспортирует незашифрованную SQLite-базу в отдельный SQLCipher-файл.
/// Не изменяет исходную базу и не перезаписывает целевой файл.
pub fn export_plaintext_to_encrypted(source_path: &Path, destination_path: &Path, password: &str) -> Result<(), String> {
    if password.is_empty() {
        return Err("Пароль не может быть пустым".into());
    }
    if !has_plaintext_sqlite_header(source_path)? {
        return Err("Исходный файл не является незашифрованной SQLite-базой".into());
    }
    let parent = destination_path.parent().ok_or("Не найден каталог назначения")?;
    let filename = destination_path.file_name().ok_or("Не найдено имя целевого файла")?.to_string_lossy();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let temporary = parent.join(format!(".{filename}.exporting-{}-{unique}", std::process::id()));
    let snapshot = parent.join(format!(".{filename}.snapshot-{}-{unique}", std::process::id()));
    let mut temporary_owned = false;
    let mut snapshot_owned = false;
    let result = (|| -> Result<(), String> {
        let original = Connection::open_with_flags(source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("Не удалось открыть исходную базу для чтения: {e}"))?;
        let reservation = std::fs::OpenOptions::new().write(true).create_new(true).open(&snapshot)
            .map_err(|e| format!("Не удалось зарезервировать путь снимка: {e}"))?;
        snapshot_owned = true;
        drop(reservation);
        original.execute("VACUUM INTO ?1", [snapshot.to_string_lossy().as_ref()])
            .map_err(|e| format!("Не удалось создать снимок SQLite: {e}"))?;
        drop(original);
        let source = Connection::open_with_flags(&snapshot, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|e| format!("Не удалось открыть временный снимок: {e}"))?;
        let snapshot_integrity: String = source.query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|e| format!("Не удалось проверить снимок SQLite: {e}"))?;
        if snapshot_integrity != "ok" {
            return Err(format!("Нарушена целостность снимка SQLite: {snapshot_integrity}"));
        }
        let new_file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary)
            .map_err(|e| format!("Не удалось создать временный файл: {e}"))?;
        temporary_owned = true;
        drop(new_file);
        source.execute("ATTACH DATABASE ?1 AS encrypted KEY ?2",
            rusqlite::params![temporary.to_string_lossy().as_ref(), password])
            .map_err(|e| format!("Не удалось подключить SQLCipher-файл: {e}"))?;
        let export_result = source.query_row("SELECT sqlcipher_export('encrypted')", [], |_| Ok(()))
            .map_err(|e| format!("Ошибка экспорта в SQLCipher: {e}"));
        export_result?;
        for pragma in ["user_version", "application_id"] {
            let value: i64 = source.query_row(&format!("PRAGMA {pragma}"), [], |row| row.get(0))
                .map_err(|e| format!("Не удалось прочитать {pragma}: {e}"))?;
            source.execute_batch(&format!("PRAGMA encrypted.{pragma} = {value}"))
                .map_err(|e| format!("Не удалось перенести {pragma}: {e}"))?;
        }

        source.execute_batch("DETACH DATABASE encrypted")
            .map_err(|e| format!("Не удалось отключить SQLCipher-файл: {e}"))?;

        drop(source);
        let source_for_verification = Connection::open_with_flags(&snapshot, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("Не удалось открыть снимок для проверки: {e}"))?;
        let verified = open_verified_database(&temporary, password)?;
        let snapshot_tables = list_user_tables(&source_for_verification)?;
        let encrypted_tables = list_user_tables(&verified)?;
        if snapshot_tables != encrypted_tables {
            return Err(format!("Список таблиц после экспорта не совпадает: SQLite={snapshot_tables:?}, SQLCipher={encrypted_tables:?}"));
        }
        let original_schema = read_schema_objects(&source_for_verification)?;
        let encrypted_schema = read_schema_objects(&verified)?;
        if original_schema != encrypted_schema {
            return Err("Структура SQLite и SQLCipher не совпадает после экспорта".into());
        }
        for pragma in ["user_version", "application_id"] {
            let sql = format!("PRAGMA {pragma}");
            let original: i64 = source_for_verification.query_row(&sql, [], |row| row.get(0)).map_err(|e| e.to_string())?;
            let encrypted: i64 = verified.query_row(&sql, [], |row| row.get(0)).map_err(|e| e.to_string())?;
            if original != encrypted {
                return Err(format!("Метаданные {pragma} не совпадают: SQLite={original}, SQLCipher={encrypted}"));
            }
        }
        let sequence_exists: i64 = source_for_verification.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = ?1 AND name = ?2", ["table", "sqlite_sequence"], |row| row.get(0)).map_err(|e| e.to_string())?;
        if sequence_exists != 0 {
            let mut original_sequence = read_table_rows(&source_for_verification, "sqlite_sequence")?;
            let mut encrypted_sequence = read_table_rows(&verified, "sqlite_sequence")?;
            original_sequence.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
            encrypted_sequence.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
            if original_sequence != encrypted_sequence {
                return Err("Счётчики AUTOINCREMENT не совпадают после экспорта".into());
            }
        }
        for table in &snapshot_tables {
            let mut original_rows = read_table_rows(&source_for_verification, table)?;
            let mut encrypted_rows = read_table_rows(&verified, table)?;
            original_rows.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
            encrypted_rows.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
            if original_rows != encrypted_rows {
                return Err(format!("Данные таблицы {table} не совпадают после экспорта"));
            }
        }
        drop(source_for_verification);
        drop(verified);
        std::fs::hard_link(&temporary, destination_path)
            .map_err(|e| format!("Не удалось опубликовать базу без перезаписи: {e}"))?;
        Ok(())
    })();
    let cleanup = if temporary_owned {
        std::fs::remove_file(&temporary).map_err(|e| format!("Не удалось удалить временный файл {}: {e}", temporary.display()))
    } else {
        Ok(())
    };
    let snapshot_cleanup = if snapshot_owned && snapshot.exists() {
        std::fs::remove_file(&snapshot).map_err(|e| format!("Не удалось удалить временный снимок {}: {e}", snapshot.display()))
    } else { Ok(()) };
    let cleanup = match (cleanup, snapshot_cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(e), Ok(())) | (Ok(()), Err(e)) => Err(e),
        (Err(a), Err(b)) => Err(format!("{a}; {b}")),
    };
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(format!("{error}; {cleanup_error}")),
        (Ok(()), Err(cleanup_error)) => Err(format!("База опубликована, но {cleanup_error}")),
    }
}

/// Создаёт новую SQLCipher-базу, не перезаписывая существующий файл.
pub fn create_encrypted_database(path: &Path, password: &str) -> Result<Connection, String> {
    if password.is_empty() {
        return Err("Пароль не может быть пустым".into());
    }

    let _new_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("Не удалось безопасно создать файл базы: {e}"))?;

    initialize_existing_empty_database(path, password)
}

fn initialize_existing_empty_database(path: &Path, password: &str) -> Result<Connection, String> {
    let mut conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE,
    )
    .map_err(|e| format!("Не удалось создать новую базу: {e}"))?;

    conn.pragma_update(None, "key", password)
        .map_err(|e| e.to_string())?;
    crate::secure_migrations::run_pending_migrations(&mut conn)?;

    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if integrity != "ok" {
        return Err(format!("Ошибка целостности новой базы: {integrity}"));
    }

    Ok(conn)
}

/// Готовит новую SQLCipher-базу и публикует её без перезаписи существующей.
pub fn initialize_encrypted_database(path: &Path, password: &str) -> Result<(), String> {
    if password.is_empty() {
        return Err("Пароль не может быть пустым".into());
    }

    let parent = path.parent().ok_or("Не найден каталог базы данных")?;
    let filename = path.file_name().ok_or("Не найдено имя базы данных")?.to_string_lossy();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temporary = parent.join(format!(".{filename}.initializing-{}-{unique}", std::process::id()));

    let mut temporary_owned = false;
    let result = (|| -> Result<(), String> {
        let _new_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| format!("Не удалось безопасно создать временный файл: {e}"))?;
        temporary_owned = true;
        drop(_new_file);
        let conn = initialize_existing_empty_database(&temporary, password)?;
        drop(conn);
        let verified = open_verified_database(&temporary, password)?;
        drop(verified);
        std::fs::hard_link(&temporary, path)
            .map_err(|e| format!("Не удалось опубликовать новую базу без перезаписи: {e}"))?;
        Ok(())
    })();

    let cleanup = if temporary_owned && temporary.exists() {
        std::fs::remove_file(&temporary).map_err(|e| {
            format!("Не удалось удалить временный файл {}: {e}", temporary.display())
        })
    } else {
        Ok(())
    };

    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(format!("{error}; {cleanup_error}")),
        (Ok(()), Err(cleanup_error)) => Err(format!("База создана, но {cleanup_error}")),
    }
}

#[cfg(test)]
mod plaintext_export_tests {
    use super::{export_plaintext_to_encrypted, has_plaintext_sqlite_header, open_verified_database};
    use rusqlite::Connection;

    #[test]
    fn exports_plaintext_without_modifying_source() {
        let directory = std::env::temp_dir().join(format!(
            "maybe-later-export-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let source_path = directory.join("source.db");
        let encrypted_path = directory.join("encrypted.db");
        let password = "test-only-export-password";

        let result = (|| -> Result<(), String> {
            let source = Connection::open(&source_path).map_err(|e| e.to_string())?;
            source.execute_batch("PRAGMA journal_mode=WAL;").map_err(|e| e.to_string())?;
            source.execute_batch("PRAGMA user_version = 42; PRAGMA application_id = 123456;")
                .map_err(|e| e.to_string())?;

            source.execute_batch("CREATE TABLE entries (id TEXT PRIMARY KEY, content TEXT NOT NULL); INSERT INTO entries VALUES ('entry-1', 'test-content');")
                .map_err(|e| e.to_string())?;
            source.execute_batch("CREATE TABLE _sqlx_migrations (version INTEGER PRIMARY KEY, checksum BLOB NOT NULL); INSERT INTO _sqlx_migrations VALUES (1, X'010203');").map_err(|e| e.to_string())?;
            source.execute_batch("CREATE TABLE test_values (id INTEGER PRIMARY KEY, value BLOB); INSERT INTO test_values VALUES (1, NULL); INSERT INTO test_values VALUES (2, X'00FF80AB');")
                .map_err(|e| e.to_string())?;
            source.execute_batch("CREATE INDEX idx_entries_content ON entries(content); CREATE VIEW entries_view AS SELECT id, content FROM entries; CREATE TABLE entry_audit (entry_id TEXT NOT NULL); CREATE TRIGGER trg_entries_audit AFTER INSERT ON entries BEGIN INSERT INTO entry_audit(entry_id) VALUES (NEW.id); END;")
                .map_err(|e| e.to_string())?;
            source.execute_batch("CREATE TABLE sequence_test (id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT); INSERT INTO sequence_test (id, value) VALUES (100, 'temporary'); DELETE FROM sequence_test;")
                .map_err(|e| e.to_string())?;
            assert!(has_plaintext_sqlite_header(&source_path)?);
            let reader = Connection::open(&source_path).map_err(|e| e.to_string())?;
            reader.execute_batch("BEGIN").map_err(|e| e.to_string())?;
            let _: i64 = reader.query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0)).map_err(|e| e.to_string())?;
            source.execute("INSERT INTO entries (id, content) VALUES (?1, ?2)", ["entry-after-reader", "wal-content"]).map_err(|e| e.to_string())?;
            let wal_path = directory.join("source.db-wal");
            assert!(std::fs::metadata(&wal_path).map_err(|e| e.to_string())?.len() > 0, "WAL-файл должен существовать и содержать данные");
            export_plaintext_to_encrypted(&source_path, &encrypted_path, password)?;
            let leftovers = std::fs::read_dir(&directory).map_err(|e| e.to_string())?
                .map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned()).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            assert!(!leftovers.iter().any(|name| name.contains(".snapshot-") || name.contains(".exporting-")), "Остались временные файлы: {leftovers:?}");
            assert!(!has_plaintext_sqlite_header(&encrypted_path)?);
            assert!(export_plaintext_to_encrypted(&source_path, &encrypted_path, password).is_err());
            assert!(open_verified_database(&encrypted_path, "wrong-password").is_err());

            let encrypted = open_verified_database(&encrypted_path, password)?;
            let history: (i64, Vec<u8>) = encrypted.query_row("SELECT version, checksum FROM _sqlx_migrations", [], |row| Ok((row.get(0)?, row.get(1)?))).map_err(|e| e.to_string())?;
            assert_eq!(history, (1, vec![1, 2, 3]));
            let content: String = encrypted.query_row("SELECT content FROM entries WHERE id = ?1", ["entry-1"], |row| row.get(0))
                .map_err(|e| e.to_string())?;
            assert_eq!(content, "test-content");
            let wal_content: String = encrypted.query_row("SELECT content FROM entries WHERE id = ?1", ["entry-after-reader"], |row| row.get(0)).map_err(|e| e.to_string())?;
            assert_eq!(wal_content, "wal-content");
            let null_value: Option<Vec<u8>> = encrypted.query_row("SELECT value FROM test_values WHERE id = 1", [], |row| row.get(0)).map_err(|e| e.to_string())?;
            assert_eq!(null_value, None);
            let blob_value: Vec<u8> = encrypted.query_row("SELECT value FROM test_values WHERE id = 2", [], |row| row.get(0)).map_err(|e| e.to_string())?;
            assert_eq!(blob_value, vec![0x00, 0xFF, 0x80, 0xAB]);
            let sequence: i64 = encrypted.query_row("SELECT seq FROM sqlite_sequence WHERE name = ?1", ["sequence_test"], |row| row.get(0))
                .map_err(|e| format!("Не удалось прочитать счётчик AUTOINCREMENT: {e}"))?;
            assert_eq!(sequence, 100, "Счётчик AUTOINCREMENT потерян при экспорте");
            drop(encrypted);

            reader.execute_batch("ROLLBACK").map_err(|e| e.to_string())?;
            drop(reader);
            drop(source);
            let original = Connection::open(&source_path).map_err(|e| e.to_string())?;
            let original_content: String = original.query_row("SELECT content FROM entries WHERE id = ?1", ["entry-1"], |row| row.get(0))
                .map_err(|e| e.to_string())?;
            assert_eq!(original_content, "test-content");
            let original_wal_content: String = original.query_row("SELECT content FROM entries WHERE id = ?1", ["entry-after-reader"], |row| row.get(0)).map_err(|e| e.to_string())?;
            assert_eq!(original_wal_content, "wal-content");
            Ok(())
        })();

        if result.is_ok() {
            std::fs::remove_dir_all(&directory).unwrap();
        }
        result.expect("Экспорт SQLite в SQLCipher должен сохранить исходные данные");
    }
}

#[cfg(test)]
mod creation_safety_tests {
    use super::create_encrypted_database;

    #[test]
    fn publishes_initialized_database_without_overwriting() {
        let path = std::env::temp_dir().join(format!(
            "maybe-later-publish-test-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let result = (|| -> Result<(), String> {
            super::initialize_encrypted_database(&path, "test-only-password")?;
            let conn = super::open_verified_database(&path, "test-only-password")?;
            let count: i64 = conn.query_row("SELECT COUNT(*) FROM _sqlx_migrations", [], |row| row.get(0)).map_err(|e| e.to_string())?;
            assert_eq!(count, 6);
            drop(conn);
            assert!(super::initialize_encrypted_database(&path, "another-password").is_err());
            assert!(super::open_verified_database(&path, "test-only-password").is_ok());
            Ok(())
        })();
        if path.exists() { std::fs::remove_file(&path).unwrap(); }
        result.expect("Безопасная публикация SQLCipher-базы должна работать");
    }

    #[test]
    fn creates_encrypted_database_and_checks_password() {
        let path = std::env::temp_dir().join(format!(
            "maybe-later-creation-test-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let result = (|| -> Result<(), String> {
            let conn = create_encrypted_database(&path, "test-only-password")?;
            conn.execute_batch("CREATE TABLE encryption_test (value TEXT NOT NULL);")
                .map_err(|e| e.to_string())?;
            conn.execute("INSERT INTO encryption_test VALUES (?1)", ["secret-value"])
                .map_err(|e| e.to_string())?;
            drop(conn);

            assert!(super::open_verified_database(&path, "wrong-password").is_err());
            let conn = super::open_verified_database(&path, "test-only-password")?;
            let value: String = conn.query_row("SELECT value FROM encryption_test", [], |row| row.get(0))
                .map_err(|e| e.to_string())?;
            assert_eq!(value, "secret-value");
            Ok(())
        })();

        if path.exists() { std::fs::remove_file(&path).unwrap(); }
        result.expect("Создание и повторное открытие SQLCipher-базы должны работать");
    }

    #[test]
    fn refuses_to_overwrite_existing_file() {
        let path = std::env::temp_dir().join(format!(
            "maybe-later-creation-safety-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        std::fs::write(&path, b"existing-file-must-survive").unwrap();

        let result = create_encrypted_database(&path, "test-only-password");
        let contents = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert!(result.is_err(), "Создание поверх существующего файла должно завершаться ошибкой");
        assert_eq!(contents, b"existing-file-must-survive");
    }
}

#[cfg(test)]
mod tests {
    use super::open_verified_database;
    use std::path::Path;

    #[test]
    fn wrong_password_is_rejected() {
        let path = Path::new("../../sqlcipher-build-test/test-data/encrypted-test.db");
        let result = open_verified_database(path, "wrong-test-password");
        assert!(result.is_err(), "Неправильный пароль должен быть отклонён");
    }

    #[test]
    fn correct_password_opens_encrypted_database() {
        let path = Path::new(
            "../../sqlcipher-build-test/test-data/encrypted-test.db"
        );

        let conn = open_verified_database(path, "correct-test-password")
            .expect("Правильный пароль должен открывать тестовую базу");

        let message: String = conn
            .query_row("SELECT message FROM test LIMIT 1", [], |row| row.get(0))
            .expect("Тестовая запись должна читаться");

        assert_eq!(message, "Secret test message");
    }
}

#[cfg(test)]
mod write_tests {
    use super::open_verified_database;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn verified_connection_allows_writing_to_encrypted_copy() {
        let source = Path::new("../../sqlcipher-build-test/test-data/encrypted-test.db");

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let temp_path = std::env::temp_dir().join(format!(
            "maybe-later-sqlcipher-write-test-{}-{unique}.db",
            std::process::id()
        ));

        std::fs::copy(source, &temp_path)
            .expect("Не удалось создать временную копию тестовой базы");

        let result = (|| -> Result<(), String> {
            let conn = open_verified_database(
                &temp_path,
                "correct-test-password",
            )?;

            conn.execute(
                "INSERT INTO test (message) VALUES (?1)",
                ["Temporary encrypted write test"],
            )
            .map_err(|e| e.to_string())?;

            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM test WHERE message = ?1",
                    ["Temporary encrypted write test"],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;

            assert_eq!(count, 1);
            Ok(())
        })();

        std::fs::remove_file(&temp_path).expect("Не удалось удалить временную тестовую базу");
        result.expect("Запись в зашифрованную копию должна работать");
    }
}
