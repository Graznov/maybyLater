use tauri::Manager;
use serde::Deserialize;
use serde::Serialize;

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct BackupEntry { id: String, title: String, content: String, date: String, tags: Vec<String>, created_at: String, updated_at: String }
#[derive(Deserialize, Serialize, Clone)]
struct BackupTag { id: String, name: String, color: String }
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct BackupPayload { entries: Vec<BackupEntry>, tags: Vec<BackupTag>, preferences: Option<serde_json::Value>, calendar_colors: Option<std::collections::BTreeMap<String, String>> }

fn verify_preferences(conn: &rusqlite::Connection, payload: &BackupPayload) -> Result<(), String> {
    if let Some(expected) = &payload.preferences {
        let raw: String = conn.query_row("SELECT value FROM preferences WHERE key=?1", ["preferences.v1"], |r| r.get(0)).map_err(|e| e.to_string())?;
        let actual: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        if &actual != expected { return Err("Настройки после восстановления не совпадают".into()); }
    }
    Ok(())
}

fn verify_calendar_colors(conn: &rusqlite::Connection, payload: &BackupPayload) -> Result<(), String> {
    if let Some(expected) = &payload.calendar_colors {
        let mut stmt = conn.prepare("SELECT date, color FROM calendar_colors ORDER BY date").map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        let actual = rows.collect::<rusqlite::Result<std::collections::BTreeMap<String, String>>>()
            .map_err(|e| e.to_string())?;
        if &actual != expected { return Err("Цвета календаря после восстановления не совпадают".into()); }
    }
    Ok(())
}

// Shared implementation for real restore and isolated in-memory verification.
fn apply_backup(conn: &mut rusqlite::Connection, payload: &BackupPayload) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM entries", []).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM tags", []).map_err(|e| e.to_string())?;
    for tag in &payload.tags {
        tx.execute("INSERT INTO tags (id, name, color) VALUES (?1, ?2, ?3)",
            rusqlite::params![tag.id, tag.name, tag.color]).map_err(|e| e.to_string())?;
    }
    for entry in &payload.entries {
        let tags = serde_json::to_string(&entry.tags).map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO entries (id, title, content, date, tags, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![entry.id, entry.title, entry.content, entry.date, tags, entry.created_at, entry.updated_at])
            .map_err(|e| e.to_string())?;
    }
    if let Some(preferences) = &payload.preferences {
        let json = serde_json::to_string(preferences).map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO preferences (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", rusqlite::params!["preferences.v1", json]).map_err(|e| e.to_string())?;
    }
    if let Some(colors) = &payload.calendar_colors {
        tx.execute("DELETE FROM calendar_colors", []).map_err(|e| e.to_string())?;
        for (date, color) in colors {
            tx.execute("INSERT INTO calendar_colors (date, color) VALUES (?1, ?2)",
                rusqlite::params![date, color]).map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())
}

#[tauri::command]
fn restore_sqlite_backup(app: tauri::AppHandle, payload: BackupPayload) -> Result<String, String> {
    let path = app.path().app_config_dir().map_err(|e| e.to_string())?.join("maybe-later.db");
    if !path.exists() { return Err("Файл базы SQLite не найден".into()); }
    let mut conn = rusqlite::Connection::open(&path).map_err(|e| e.to_string())?;
    conn.busy_timeout(std::time::Duration::from_secs(10)).map_err(|e| e.to_string())?;
    // A SQLite-consistent, separate on-disk snapshot is created BEFORE any changes.
    // VACUUM INTO includes committed WAL data and fails if the destination exists.
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?.as_nanos();
    let snapshot = path.with_file_name(format!("maybe-later-before-restore-{timestamp}.db"));
    conn.execute("VACUUM INTO ?1", [snapshot.to_string_lossy().as_ref()])
        .map_err(|e| format!("Не удалось создать защитную копию SQLite: {e}"))?;
    apply_backup(&mut conn, &payload)?;
    Ok(snapshot.to_string_lossy().into_owned())
}

// Creates a persistent, SQLite-consistent safety snapshot and verifies it can be opened.
// No writes are made to the live database. The snapshot remains available for recovery.
#[tauri::command]
fn test_sqlite_safety_snapshot(app: tauri::AppHandle) -> Result<String, String> {
    let path = app.path().app_config_dir().map_err(|e| e.to_string())?.join("maybe-later.db");
    if !path.is_file() { return Err("Файл базы SQLite не найден".into()); }
    let source = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    source.busy_timeout(std::time::Duration::from_secs(10)).map_err(|e| e.to_string())?;
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?.as_nanos();
    let snapshot = path.with_file_name(format!("maybe-later-safety-check-{timestamp}.db"));
    source.execute("VACUUM INTO ?1", [snapshot.to_string_lossy().as_ref()])
        .map_err(|e| format!("Не удалось создать защитную копию: {e}"))?;
    let result = (|| -> Result<(i64, i64), String> {
        let copy = rusqlite::Connection::open_with_flags(&snapshot, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("Копия не открывается: {e}"))?;
        let integrity: String = copy.query_row("PRAGMA integrity_check", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        if integrity != "ok" { return Err(format!("Ошибка целостности копии: {integrity}")); }
        // Compare every persisted field, not just the counts.
        for (table, columns) in [
            ("entries", "id, title, content, date, tags, created_at, updated_at"),
            ("tags", "id, name, color"),
            ("preferences", "key, value"),
            ("calendar_colors", "date, color"),
        ] {
            // preferences is keyed by `key`, unlike entries and tags (keyed by `id`).
            let order_column = match table { "preferences" => "key", "calendar_colors" => "date", _ => "id" };
            let sql = format!("SELECT {columns} FROM {table} ORDER BY {order_column}");
            let read = |conn: &rusqlite::Connection| -> Result<Vec<Vec<String>>, String> {
                let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
                let cols = stmt.column_count();
                let rows = stmt.query_map([], |row| (0..cols).map(|i| row.get::<_, String>(i)).collect::<rusqlite::Result<Vec<_>>>())
                    .map_err(|e| e.to_string())?;
                rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())
            };
            if read(&source)? != read(&copy)? { return Err(format!("Данные {table} в копии не совпадают с основной базой")); }
        }
        let entries: i64 = copy.query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        let tags: i64 = copy.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        Ok((entries, tags))
    })();
    match result {
        Ok((entries, tags)) => Ok(format!("Защитная копия создана и прочитана: {entries} записей, {tags} тегов. Все поля совпадают, integrity_check: ok. Файл: {}", snapshot.display())),
        Err(error) => Err(format!("Проверка защитной копии не прошла: {error}. Файл: {}", snapshot.display())),
    }
}

// Test the production restore against a real on-disk COPY of the live database.
// The original database is read for the snapshot only; no changes are applied to it.
#[tauri::command]
fn test_sqlite_restore_on_copy(app: tauri::AppHandle, payload: BackupPayload) -> Result<String, String> {
    let path = app.path().app_config_dir().map_err(|e| e.to_string())?.join("maybe-later.db");
    if !path.exists() { return Err("Файл базы SQLite не найден".into()); }
    let source = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    source.busy_timeout(std::time::Duration::from_secs(10)).map_err(|e| e.to_string())?;
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?.as_nanos();
    let copy_path = path.with_file_name(format!("maybe-later-restore-test-{timestamp}.db"));
    // Remove the temporary file even when any verification step fails.
    struct TemporaryDb(std::path::PathBuf);
    impl Drop for TemporaryDb { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); } }
    let _cleanup = TemporaryDb(copy_path.clone());
    source.execute("VACUUM INTO ?1", [copy_path.to_string_lossy().as_ref()])
        .map_err(|e| format!("Не удалось создать тестовую копию: {e}"))?;
    drop(source);
    let mut copy = rusqlite::Connection::open(&copy_path).map_err(|e| e.to_string())?;
    copy.busy_timeout(std::time::Duration::from_secs(10)).map_err(|e| e.to_string())?;
    apply_backup(&mut copy, &payload)?;
    let entry_count: i64 = copy.query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let tag_count: i64 = copy.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if entry_count as usize != payload.entries.len() || tag_count as usize != payload.tags.len() {
        return Err("Число записей или тегов после восстановления не совпадает".into());
    }
    // Verify the restored content, not just the row counts.
    for entry in &payload.entries {
        let stored: (String, String, String, String, String, String) = copy.query_row(
            "SELECT title, content, date, tags, created_at, updated_at FROM entries WHERE id=?1",
            [&entry.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        ).map_err(|e| e.to_string())?;
        if stored != (entry.title.clone(), entry.content.clone(), entry.date.clone(),
            serde_json::to_string(&entry.tags).map_err(|e| e.to_string())?,
            entry.created_at.clone(), entry.updated_at.clone()) {
            return Err(format!("Не совпадает запись {}", entry.id));
        }
    }
    for tag in &payload.tags {
        let stored: (String, String) = copy.query_row(
            "SELECT name, color FROM tags WHERE id=?1", [&tag.id], |r| Ok((r.get(0)?, r.get(1)?))
        ).map_err(|e| e.to_string())?;
        if stored != (tag.name.clone(), tag.color.clone()) { return Err(format!("Не совпадает тег {}", tag.id)); }
    }
    verify_preferences(&copy, &payload)?;
    verify_calendar_colors(&copy, &payload)?;
    drop(copy);
    Ok(format!("Тест на дисковой копии пройден: {} записей, {} тегов. Основная база не изменялась.", entry_count, tag_count))
}

// Uses an isolated RAM database; never opens or modifies maybe-later.db.
#[tauri::command]
fn verify_sqlite_restore(payload: BackupPayload) -> Result<String, String> {
    let mut conn = rusqlite::Connection::open_in_memory().map_err(|e| e.to_string())?;
    conn.execute_batch("CREATE TABLE entries (id TEXT PRIMARY KEY NOT NULL, title TEXT NOT NULL, content TEXT NOT NULL, date TEXT NOT NULL, tags TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL); CREATE TABLE tags (id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL, color TEXT NOT NULL); CREATE TABLE preferences (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL); CREATE TABLE calendar_colors (date TEXT PRIMARY KEY NOT NULL, color TEXT NOT NULL);")
        .map_err(|e| e.to_string())?;
    apply_backup(&mut conn, &payload)?;
    let entry_count: i64 = conn.query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let tag_count: i64 = conn.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if entry_count as usize != payload.entries.len() || tag_count as usize != payload.tags.len() {
        return Err("Количество записей или тегов не совпадает".into());
    }
    for entry in &payload.entries {
        let stored: (String, String, String, String, String, String) = conn.query_row(
            "SELECT title, content, date, tags, created_at, updated_at FROM entries WHERE id=?1",
            [&entry.id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))
        ).map_err(|e| e.to_string())?;
        if stored != (entry.title.clone(), entry.content.clone(), entry.date.clone(), serde_json::to_string(&entry.tags).map_err(|e| e.to_string())?, entry.created_at.clone(), entry.updated_at.clone()) {
            return Err(format!("Несовпадение полей записи {}", entry.id));
        }
    }
    for tag in &payload.tags {
        let stored: (String, String) = conn.query_row("SELECT name, color FROM tags WHERE id=?1", [&tag.id], |r| Ok((r.get(0)?,r.get(1)?))).map_err(|e| e.to_string())?;
        if stored != (tag.name.clone(), tag.color.clone()) { return Err(format!("Несовпадение тега {}", tag.id)); }
    }
    verify_preferences(&conn, &payload)?;
    verify_calendar_colors(&conn, &payload)?;
    // Force a UNIQUE violation midway through a second restore, then verify rollback.
    // This exercises the same apply_backup function used by production restore.
    if let Some(first) = payload.entries.first() {
        let mut invalid = payload.clone();
        invalid.entries.push(first.clone());
        if apply_backup(&mut conn, &invalid).is_ok() { return Err("Не сработала проверка отката транзакции".into()); }
        let after: i64 = conn.query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        if after != entry_count { return Err("После ошибки транзакция изменила тестовую базу".into()); }
        verify_preferences(&conn, &payload)?;
        verify_calendar_colors(&conn, &payload)?;
    }
    Ok(format!("Проверено {} записей и {} тегов; содержимое совпадает, откат транзакции работает. Основная база не затронута.", entry_count, tag_count))
}

use tauri_plugin_sql::{Migration, MigrationKind};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let migrations = vec![
        Migration {
            version: 1,
            description: "create app metadata table",
            sql: "CREATE TABLE IF NOT EXISTS app_metadata (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "create diary entries table",
            sql: "
                CREATE TABLE IF NOT EXISTS entries (
                    id TEXT PRIMARY KEY NOT NULL,
                    title TEXT NOT NULL,
                    content TEXT NOT NULL,
                    date TEXT NOT NULL,
                    tags TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_entries_date ON entries(date);
            ",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 3,
            description: "create tags table",
            sql: "CREATE TABLE IF NOT EXISTS tags (id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL, color TEXT NOT NULL);",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 4,
            description: "atomic tag changes via SQLite triggers",
            sql: r#"
                CREATE TRIGGER IF NOT EXISTS trg_tag_rename
                AFTER UPDATE OF name ON tags
                WHEN OLD.name <> NEW.name
                BEGIN
                    UPDATE entries
                    SET tags = (
                        SELECT json_group_array(value)
                        FROM (
                            SELECT CASE WHEN value = OLD.name THEN NEW.name ELSE value END AS value
                            FROM json_each(entries.tags)
                            ORDER BY CAST(key AS INTEGER)
                        )
                    ), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                    WHERE EXISTS (SELECT 1 FROM json_each(entries.tags) WHERE value = OLD.name);
                END;
                CREATE TRIGGER IF NOT EXISTS trg_tag_delete
                AFTER DELETE ON tags
                BEGIN
                    UPDATE entries
                    SET tags = (
                        SELECT json_group_array(value)
                        FROM (
                            SELECT value FROM json_each(entries.tags)
                            WHERE value <> OLD.name ORDER BY CAST(key AS INTEGER)
                        )
                    ), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                    WHERE EXISTS (SELECT 1 FROM json_each(entries.tags) WHERE value = OLD.name);
                END;
            "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 5,
            description: "create preferences snapshot table",
            sql: "CREATE TABLE IF NOT EXISTS preferences (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 6,
            description: "create calendar date colors table",
            sql: "CREATE TABLE IF NOT EXISTS calendar_colors (date TEXT PRIMARY KEY NOT NULL, color TEXT NOT NULL);",
            kind: MigrationKind::Up,
        },
    ];

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![restore_sqlite_backup, verify_sqlite_restore, test_sqlite_restore_on_copy, test_sqlite_safety_snapshot])
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations("sqlite:maybe-later.db", migrations)
                .build(),
        )
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
