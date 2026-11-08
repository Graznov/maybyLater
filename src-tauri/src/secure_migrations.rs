use rusqlite::OptionalExtension;
use sha2::{Digest, Sha384};

const CREATE_MIGRATIONS_TABLE: &str = "CREATE TABLE IF NOT EXISTS _sqlx_migrations (version BIGINT PRIMARY KEY, description TEXT NOT NULL, installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, success BOOLEAN NOT NULL, checksum BLOB NOT NULL, execution_time BIGINT NOT NULL)";

fn migration_checksum(sql: &str) -> Vec<u8> {
    Sha384::digest(sql.as_bytes()).to_vec()
}

#[cfg(test)]
mod tests {
    use super::migration_checksum;

    #[test]
    fn checksum_matches_sqlx_migration_one() {
        let migration = &crate::schema_migrations::migrations()[0];
        let checksum = migration_checksum(migration.sql);

        assert_eq!(checksum.len(), 48);
        assert_eq!(
            checksum.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
            "166b838931ee08f1f1325c7cd041a0f2ed595e4a7aac92f41e316abc1f0ffacc7b0cb520cd45b710bb920b1a53f24c18"
        );
    }
}

fn check_no_failed_migrations(conn: &rusqlite::Connection) -> Result<(), String> {
    let failed_version: Option<i64> = conn
        .query_row(
            "SELECT version FROM _sqlx_migrations WHERE success = false ORDER BY version LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some(version) = failed_version {
        return Err(format!("Обнаружена незавершённая миграция SQLx: {version}"));
    }

    Ok(())
}

#[cfg(test)]
mod failed_migration_tests {
    use super::{check_no_failed_migrations, CREATE_MIGRATIONS_TABLE};
    use rusqlite::Connection;

    #[test]
    fn rejects_failed_sqlx_migration() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_MIGRATIONS_TABLE).unwrap();

        conn.execute(
            "INSERT INTO _sqlx_migrations \
             (version, description, success, checksum, execution_time) \
             VALUES (4, 'failed test migration', 0, X'00', -1)",
            [],
        )
        .unwrap();

        let error = check_no_failed_migrations(&conn).unwrap_err();
        assert!(error.contains("4"), "Unexpected error: {error}");
    }
}

#[cfg(test)]
mod successful_migration_tests {
    use super::{check_no_failed_migrations, CREATE_MIGRATIONS_TABLE};
    use rusqlite::Connection;

    #[test]
    fn accepts_successful_sqlx_migration() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_MIGRATIONS_TABLE).unwrap();

        conn.execute(
            "INSERT INTO _sqlx_migrations \
             (version, description, success, checksum, execution_time) \
             VALUES (1, 'successful test migration', 1, X'00', -1)",
            [],
        )
        .unwrap();

        assert!(check_no_failed_migrations(&conn).is_ok());
    }
}

#[derive(Debug, PartialEq, Eq)]
struct AppliedMigration {
    version: i64,
    checksum: Vec<u8>,
}

fn load_applied_migrations(
    conn: &rusqlite::Connection,
) -> Result<Vec<AppliedMigration>, String> {
    let mut stmt = conn
        .prepare("SELECT version, checksum FROM _sqlx_migrations ORDER BY version")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(AppliedMigration {
                version: row.get(0)?,
                checksum: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod applied_migration_tests {
    use super::{load_applied_migrations, AppliedMigration, CREATE_MIGRATIONS_TABLE};
    use rusqlite::Connection;

    #[test]
    fn loads_applied_sqlx_migrations_in_version_order() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_MIGRATIONS_TABLE).unwrap();

        conn.execute(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (?1, ?2, 1, ?3, -1)",
            (2_i64, "second", vec![0x12_u8, 0x34]),
        ).unwrap();

        conn.execute(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (?1, ?2, 1, ?3, -1)",
            (1_i64, "first", vec![0xAB_u8, 0xCD]),
        ).unwrap();

        assert_eq!(
            load_applied_migrations(&conn).unwrap(),
            vec![
                AppliedMigration { version: 1, checksum: vec![0xAB, 0xCD] },
                AppliedMigration { version: 2, checksum: vec![0x12, 0x34] },
            ]
        );
    }
}

fn validate_applied_migrations(
    applied: &[AppliedMigration],
) -> Result<(), String> {
    let migrations = crate::schema_migrations::migrations();

    for migration in applied {
        let expected = migrations
            .iter()
            .find(|candidate| candidate.version == migration.version)
            .ok_or_else(|| {
                format!("Неизвестная версия миграции SQLx: {}", migration.version)
            })?;

        if migration.checksum != migration_checksum(expected.sql) {
            return Err(format!(
                "Несовпадение SHA-384 миграции SQLx: {}",
                migration.version
            ));
        }
    }

    for (index, migration) in applied.iter().enumerate() {
        let expected_version = (index + 1) as i64;
        if migration.version != expected_version {
            return Err(format!("Отсутствует или нарушен порядок миграции SQLx: {expected_version}"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod validation_tests {
    use super::{migration_checksum, validate_applied_migrations, AppliedMigration};

    #[test]
    fn accepts_matching_sqlx_checksums() {
        let migrations = crate::schema_migrations::migrations();

        let applied: Vec<AppliedMigration> = migrations
            .iter()
            .take(2)
            .map(|migration| AppliedMigration {
                version: migration.version,
                checksum: migration_checksum(migration.sql),
            })
            .collect();

        assert!(validate_applied_migrations(&applied).is_ok());
    }
}

#[cfg(test)]
mod unknown_version_tests {
    use super::{validate_applied_migrations, AppliedMigration};

    #[test]
    fn rejects_unknown_sqlx_migration_version() {
        let applied = vec![AppliedMigration {
            version: 999,
            checksum: vec![0; 48],
        }];

        let error = validate_applied_migrations(&applied).unwrap_err();

        assert!(
            error.contains("999"),
            "Unexpected error: {error}"
        );
    }
}

#[cfg(test)]
mod checksum_mismatch_tests {
    use super::{validate_applied_migrations, AppliedMigration};

    #[test]
    fn rejects_mismatched_sqlx_checksum() {
        let applied = vec![AppliedMigration {
            version: 1,
            checksum: vec![0; 48],
        }];

        let error = validate_applied_migrations(&applied).unwrap_err();

        assert!(
            error.contains("SHA-384") && error.contains("1"),
            "Unexpected error: {error}"
        );
    }
}

#[cfg(test)]
mod missing_version_tests {
    use super::{migration_checksum, validate_applied_migrations, AppliedMigration};

    #[test]
    fn rejects_missing_sqlx_migration_version() {
        let migrations = crate::schema_migrations::migrations();

        let applied: Vec<AppliedMigration> = [0_usize, 2]
            .iter()
            .map(|&index| AppliedMigration {
                version: migrations[index].version,
                checksum: migration_checksum(migrations[index].sql),
            })
            .collect();

        let error = validate_applied_migrations(&applied).unwrap_err();

        assert!(
            error.contains("2"),
            "Unexpected error: {error}"
        );
    }
}

#[cfg(test)]
mod empty_history_tests {
    use super::validate_applied_migrations;

    #[test]
    fn accepts_empty_sqlx_migration_history() {
        assert!(validate_applied_migrations(&[]).is_ok());
    }
}

fn apply_migration(
    conn: &mut rusqlite::Connection,
    migration: &crate::migration_types::Migration,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    tx.execute_batch(migration.sql)
        .map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO _sqlx_migrations \
         (version, description, success, checksum, execution_time) \
         VALUES (?1, ?2, 1, ?3, -1)",
        (
            migration.version,
            migration.description,
            migration_checksum(migration.sql),
        ),
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod apply_migration_tests {
    use super::{apply_migration, migration_checksum, CREATE_MIGRATIONS_TABLE};
    use rusqlite::Connection;

    #[test]
    fn applies_migration_and_records_sqlx_history_atomically() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_MIGRATIONS_TABLE).unwrap();

        let migrations = crate::schema_migrations::migrations();
        let migration = &migrations[0];

        apply_migration(&mut conn, migration).unwrap();

        let table_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'app_metadata'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        let (version, checksum): (i64, Vec<u8>) = conn
            .query_row(
                "SELECT version, checksum FROM _sqlx_migrations",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(table_exists, 1);
        assert_eq!(version, migration.version);
        assert_eq!(checksum, migration_checksum(migration.sql));
    }
}

#[cfg(test)]
mod migration_rollback_tests {
    use super::{apply_migration, CREATE_MIGRATIONS_TABLE};
    use rusqlite::Connection;
    use crate::migration_types::Migration;

    #[test]
    fn rolls_back_schema_and_history_when_migration_fails() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_MIGRATIONS_TABLE).unwrap();

        let migration = Migration {
            version: 1,
            description: "test rollback",
            sql: "CREATE TABLE rollback_test (id INTEGER); INSERT INTO missing_table VALUES (1);",
        };

        assert!(apply_migration(&mut conn, &migration).is_err());

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'rollback_test'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        let history_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM _sqlx_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(table_count, 0);
        assert_eq!(history_count, 0);
    }
}

#[cfg(test)]
mod duplicate_migration_tests {
    use super::{apply_migration, CREATE_MIGRATIONS_TABLE};
    use rusqlite::Connection;

    #[test]
    fn rejects_duplicate_migration_without_changing_history() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_MIGRATIONS_TABLE).unwrap();

        let migrations = crate::schema_migrations::migrations();
        let migration = &migrations[0];

        apply_migration(&mut conn, migration).unwrap();
        assert!(apply_migration(&mut conn, migration).is_err());

        let history_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM _sqlx_migrations WHERE version = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(history_count, 1);
    }
}

pub(crate) fn run_pending_migrations(conn: &mut rusqlite::Connection) -> Result<(), String> {
    conn.execute_batch(CREATE_MIGRATIONS_TABLE)
        .map_err(|e| e.to_string())?;

    check_no_failed_migrations(conn)?;

    let applied = load_applied_migrations(conn)?;
    validate_applied_migrations(&applied)?;

    let migrations = crate::schema_migrations::migrations();

    for migration in migrations.iter().skip(applied.len()) {
        apply_migration(conn, migration)?;
    }

    Ok(())
}

#[cfg(test)]
mod run_pending_migrations_tests {
    use super::{load_applied_migrations, run_pending_migrations, validate_applied_migrations};
    use rusqlite::Connection;

    #[test]
    fn applies_all_pending_migrations_to_empty_database() {
        let mut conn = Connection::open_in_memory().unwrap();

        run_pending_migrations(&mut conn).unwrap();

        let applied = load_applied_migrations(&conn).unwrap();

        assert_eq!(applied.len(), 6);
        assert!(validate_applied_migrations(&applied).is_ok());

        let entries_table_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'entries'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(entries_table_exists, 1);
    }
}

#[cfg(test)]
mod migration_idempotency_tests {
    use super::{load_applied_migrations, run_pending_migrations};
    use rusqlite::Connection;

    #[test]
    fn rerunning_migrations_preserves_existing_history() {
        let mut conn = Connection::open_in_memory().unwrap();

        run_pending_migrations(&mut conn).unwrap();
        let history_before = load_applied_migrations(&conn).unwrap();

        run_pending_migrations(&mut conn).unwrap();
        let history_after = load_applied_migrations(&conn).unwrap();

        assert_eq!(history_before.len(), 6);
        assert_eq!(history_after, history_before);
    }
}

#[cfg(test)]
mod partial_migration_tests {
    use super::{apply_migration, load_applied_migrations, run_pending_migrations, CREATE_MIGRATIONS_TABLE};
    use rusqlite::Connection;

    #[test]
    fn applies_only_missing_migrations_after_existing_prefix() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_MIGRATIONS_TABLE).unwrap();

        let migrations = crate::schema_migrations::migrations();

        for migration in migrations.iter().take(3) {
            apply_migration(&mut conn, migration).unwrap();
        }

        let history_before = load_applied_migrations(&conn).unwrap();
        assert_eq!(history_before.len(), 3);

        run_pending_migrations(&mut conn).unwrap();

        let history_after = load_applied_migrations(&conn).unwrap();

        assert_eq!(history_after.len(), 6);
        assert_eq!(&history_after[..3], history_before.as_slice());
        assert_eq!(
            history_after.iter().map(|m| m.version).collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5, 6]
        );
    }
}

#[cfg(test)]
mod corrupted_history_tests {
    use super::{apply_migration, run_pending_migrations, CREATE_MIGRATIONS_TABLE};
    use rusqlite::Connection;

    #[test]
    fn rejects_corrupted_history_before_applying_pending_migrations() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_MIGRATIONS_TABLE).unwrap();

        let migrations = crate::schema_migrations::migrations();
        apply_migration(&mut conn, &migrations[0]).unwrap();

        conn.execute(
            "UPDATE _sqlx_migrations SET checksum = X'00' WHERE version = 1",
            [],
        )
        .unwrap();

        let error = run_pending_migrations(&mut conn).unwrap_err();
        assert!(error.contains("SHA-384"), "Unexpected error: {error}");

        let history_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM _sqlx_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();

        let entries_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'entries'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(history_count, 1);
        assert_eq!(entries_table_count, 0);
    }
}

#[cfg(test)]
mod failed_history_guard_tests {
    use super::{run_pending_migrations, CREATE_MIGRATIONS_TABLE};
    use rusqlite::Connection;

    #[test]
    fn rejects_failed_history_before_applying_pending_migrations() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_MIGRATIONS_TABLE).unwrap();

        conn.execute(
            "INSERT INTO _sqlx_migrations \
             (version, description, success, checksum, execution_time) \
             VALUES (1, 'failed migration', 0, X'00', -1)",
            [],
        )
        .unwrap();

        let error = run_pending_migrations(&mut conn).unwrap_err();
        assert!(error.contains("1"), "Unexpected error: {error}");

        let entries_table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'entries'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        let history_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM _sqlx_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(entries_table_count, 0);
        assert_eq!(history_count, 1);
    }
}
