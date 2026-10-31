use rusqlite::{Connection, OpenFlags};
use std::path::Path;

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
