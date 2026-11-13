//! Crash-safe file handover for SQLCipher password changes.
//! The marker contains a backup *filename*, never a password.
use rusqlite::Connection;
use std::io::Write;
use std::path::{Path, PathBuf};

const ACTIVE: &str = "maybe-later.db";
const MARKER: &str = "maybe-later-password-change-pending";

fn read_backup_path(directory: &Path) -> Result<Option<PathBuf>, String> {
    let marker = directory.join(MARKER);
    if !marker.exists() {
        return Ok(None);
    }
    let filename = std::fs::read_to_string(&marker)
        .map_err(|e| format!("Не удалось прочитать маркер смены пароля: {e}"))?;
    let filename = filename.trim();
    // Never interpret a marker as an arbitrary filesystem path.
    if !filename.starts_with("maybe-later-before-password-change-")
        || !filename.ends_with(".db")
        || filename.contains('/')
        || filename.contains('\\')
        || filename == "maybe-later-before-password-change-.db"
    {
        return Err(
            "Некорректный маркер смены пароля; автоматическое восстановление остановлено".into(),
        );
    }
    Ok(Some(directory.join(filename)))
}

/// Called before database-presence detection and unlock.
/// If the process died between the two renames, restore the old database.
pub fn recover_interrupted_switch(directory: &Path) -> Result<(), String> {
    let Some(backup) = read_backup_path(directory)? else {
        return Ok(());
    };
    let active = directory.join(ACTIVE);
    if active.exists() {
        // The active file may be old OR new. Only a successful unlock can
        // establish which password is valid; never overwrite this file.
        return Ok(());
    }
    if !backup.is_file() {
        return Err(format!(
            "Прерванная смена пароля: основная база отсутствует, защитная копия не найдена: {}",
            backup.display()
        ));
    }
    std::fs::rename(&backup, &active).map_err(|e| {
        format!(
            "Не удалось восстановить дневник из {}: {e}",
            backup.display()
        )
    })?;
    Ok(())
}

pub fn clear_completed_marker(directory: &Path) {
    // Failure to remove the marker is harmless: next unlock re-checks it.
    let _ = std::fs::remove_file(directory.join(MARKER));
}

pub fn change_password(directory: &Path, old: &str, new: &str) -> Result<Connection, String> {
    if directory.join(MARKER).exists() {
        return Err(
            "Есть незавершённая смена пароля; перезапустите приложение и разблокируйте дневник"
                .into(),
        );
    }
    let source = directory.join(ACTIVE);
    if !source.is_file() {
        return Err("Файл дневника не найден".into());
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let backup_name = format!("maybe-later-before-password-change-{timestamp}.db");
    let backup = directory.join(&backup_name);
    let candidate = directory.join(format!("maybe-later-password-change-{timestamp}.db"));

    // This exporter verifies both databases and preserves the original.
    crate::sqlcipher_connection::change_encrypted_database_password(&source, &candidate, old, new)?;
    let result = (|| -> Result<Connection, String> {
        // A marker is durable before any rename can make the active file absent.
        let mut marker = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join(MARKER))
            .map_err(|e| format!("Не удалось создать маркер: {e}"))?;
        marker
            .write_all(backup_name.as_bytes())
            .map_err(|e| e.to_string())?;
        marker.sync_all().map_err(|e| e.to_string())?;
        drop(marker);
        // Refuse to replace an existing backup even on a timestamp collision.
        if backup.exists() {
            return Err("Защитная копия с таким именем уже существует".into());
        }
        std::fs::rename(&source, &backup)
            .map_err(|e| format!("Не удалось сохранить старую базу: {e}"))?;
        if let Err(e) = std::fs::rename(&candidate, &source) {
            // Leave marker and backup if rollback fails; startup recovery handles it.
            let restored = std::fs::rename(&backup, &source).is_ok();
            return Err(format!(
                "Не удалось установить новую базу: {e}; старая восстановлена: {restored}"
            ));
        }
        // Never delete the old encrypted database after a successful switch.
        match crate::sqlcipher_connection::open_verified_database(&source, new) {
            Ok(conn) => Ok(conn),
            Err(error) => {
                // Preserve the failed candidate and restore the known-good backup.
                // If either rename fails, leave the marker for manual recovery.
                std::fs::rename(&source, &candidate).map_err(|e| format!(
                    "Новая база не открывается ({error}); не удалось отложить её для отката: {e}; старая база: {}", backup.display()
                ))?;
                std::fs::rename(&backup, &source).map_err(|e| format!(
                    "Новая база не открывается ({error}); не удалось восстановить старую базу: {e}; старая база: {}", backup.display()
                ))?;
                Err(format!(
                    "Новая база не прошла проверку ({error}); старая база восстановлена"
                ))
            }
        }
    })();
    match result {
        Ok(conn) => {
            clear_completed_marker(directory);
            Ok(conn)
        }
        Err(error) => {
            // If active is missing, try to recover; never overwrite a present file.
            let recovery = recover_interrupted_switch(directory);
            if source.is_file() {
                // Marker may be removed only if old password still opens active.
                if crate::sqlcipher_connection::open_verified_database(&source, old).is_ok() {
                    clear_completed_marker(directory);
                }
            }
            // Keep candidate on error for inspection/recovery.
            match recovery {
                Ok(()) => Err(error),
                Err(e) => Err(format!("{error}; {e}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "mbl-password-recovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&p).unwrap();
        p
    }
    #[test]
    fn recovers_crash_between_renames() {
        let dir = temp();
        let name = "maybe-later-before-password-change-123.db";
        std::fs::write(dir.join(name), b"old encrypted bytes").unwrap();
        std::fs::write(dir.join(MARKER), name).unwrap();
        recover_interrupted_switch(&dir).unwrap();
        assert_eq!(
            std::fs::read(dir.join(ACTIVE)).unwrap(),
            b"old encrypted bytes"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn never_overwrites_published_database() {
        let dir = temp();
        let name = "maybe-later-before-password-change-123.db";
        std::fs::write(dir.join(name), b"old").unwrap();
        std::fs::write(dir.join(ACTIVE), b"new").unwrap();
        std::fs::write(dir.join(MARKER), name).unwrap();
        recover_interrupted_switch(&dir).unwrap();
        assert_eq!(std::fs::read(dir.join(ACTIVE)).unwrap(), b"new");
        assert_eq!(std::fs::read(dir.join(name)).unwrap(), b"old");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn rejects_path_traversal_in_marker() {
        let dir = temp();
        std::fs::write(
            dir.join(MARKER),
            "../maybe-later-before-password-change-123.db",
        )
        .unwrap();
        assert!(recover_interrupted_switch(&dir).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn changes_password_and_preserves_journal_data() {
        let dir = temp();
        let source = dir.join(ACTIVE);

        let result = (|| -> Result<(), String> {
            let conn = crate::sqlcipher_connection::create_encrypted_database(
                &source,
                "old-test-password",
            )?;
            conn.execute_batch(
                "CREATE TABLE password_change_test (
                    id INTEGER PRIMARY KEY,
                    value TEXT NOT NULL
                );
                INSERT INTO password_change_test (id, value)
                VALUES (1, 'preserved journal data');",
            )
            .map_err(|e| e.to_string())?;
            drop(conn);

            let changed = change_password(&dir, "old-test-password", "new-test-password")?;
            let value: String = changed
                .query_row(
                    "SELECT value FROM password_change_test WHERE id = 1",
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            assert_eq!(value, "preserved journal data");
            drop(changed);

            assert!(crate::sqlcipher_connection::open_verified_database(
                &source,
                "new-test-password"
            )
            .is_ok());
            assert!(crate::sqlcipher_connection::open_verified_database(
                &source,
                "old-test-password"
            )
            .is_err());

            let backups: Vec<_> = std::fs::read_dir(&dir)
                .map_err(|e| e.to_string())?
                .map(|entry| entry.map(|entry| entry.path()).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| {
                            name.starts_with("maybe-later-before-password-change-")
                                && name.ends_with(".db")
                        })
                })
                .collect();
            assert_eq!(backups.len(), 1);

            let backup = crate::sqlcipher_connection::open_verified_database(
                &backups[0],
                "old-test-password",
            )?;
            let backup_value: String = backup
                .query_row(
                    "SELECT value FROM password_change_test WHERE id = 1",
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            assert_eq!(backup_value, "preserved journal data");
            drop(backup);

            assert!(!dir.join(MARKER).exists());
            Ok(())
        })();

        let cleanup = std::fs::remove_dir_all(&dir);
        result.expect("Полная смена пароля должна сохранить данные");
        cleanup.expect("Тестовый каталог должен удаляться");
    }
}
