use crate::migration_types::Migration;

pub fn migrations() -> Vec<Migration> {
    let migrations = vec![
        Migration {
            version: 1,
            description: "create app metadata table",
            sql: "CREATE TABLE IF NOT EXISTS app_metadata (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);",
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
        },
        Migration {
            version: 3,
            description: "create tags table",
            sql: "CREATE TABLE IF NOT EXISTS tags (id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL, color TEXT NOT NULL);",
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
        },
        Migration {
            version: 5,
            description: "create preferences snapshot table",
            sql: "CREATE TABLE IF NOT EXISTS preferences (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);",
        },
        Migration {
            version: 6,
            description: "create calendar date colors table",
            sql: "CREATE TABLE IF NOT EXISTS calendar_colors (date TEXT PRIMARY KEY NOT NULL, color TEXT NOT NULL);",
        },
    ];

    migrations
}

#[cfg(test)]
mod tests {
    use super::migrations;
    use sha2::{Digest, Sha384};

    #[test]
    fn migration_checksums_use_sqlx_sha384_format() {
        let migrations = migrations();
        assert_eq!(migrations.len(), 6);

        for (index, migration) in migrations.iter().enumerate() {
            assert_eq!(migration.version, (index + 1) as i64);

            let checksum = Sha384::digest(migration.sql.as_bytes());
            assert_eq!(checksum.len(), 48);
            if migration.version == 1 { assert_eq!(format!("{:x}", checksum), "166b838931ee08f1f1325c7cd041a0f2ed595e4a7aac92f41e316abc1f0ffacc7b0cb520cd45b710bb920b1a53f24c18"); }
            if migration.version == 2 { assert_eq!(format!("{:x}", checksum), "39aa26b633a9d38e250b42ad297498892c103527a9ae3fc5803db700f6f01f472058dda38d7fc55b830d36df96359f02"); }
            if migration.version == 3 { assert_eq!(format!("{:x}", checksum), "7f84c5057603086a2d3f1a872c4aa4268a6d4912d3b2494394feb1a8b5bb1436aaea15ade3e5c39e2241c597269a47be"); }
            if migration.version == 4 { assert_eq!(format!("{:x}", checksum), "89115979407a99215ec714d7859ccb8b9d15ff5944966640f040276632c1c1b71b220d3b7921c8e7bf97508697df1e8d"); }
            if migration.version == 5 { assert_eq!(format!("{:x}", checksum), "0fdf146cede1597d6f189de21bfedd6fdbe464c8da55f2e5bcf484dca5842ed48993c4403983597dc0a78bf8d38af0aa"); }
            if migration.version == 6 { assert_eq!(format!("{:x}", checksum), "b5b499144100cc21e50f6950892456783efa1c9c81fd638a5ff7fdc102793bc37aec2c6b6ad42abee548bf9985e57ca2"); }
        }
    }
}
