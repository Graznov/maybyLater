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
    ];

    tauri::Builder::default()
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations("sqlite:maybe-later.db", migrations)
                .build(),
        )
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
