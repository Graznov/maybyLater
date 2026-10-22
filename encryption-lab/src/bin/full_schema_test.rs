
use rusqlite::{params, Connection, Result};
use std::collections::BTreeMap;
use std::path::Path;

fn schema(conn: &Connection) -> Result<BTreeMap<(String, String), String>> {
    let mut stmt = conn.prepare(
        "SELECT type, name, sql
         FROM sqlite_master
         WHERE type IN ('table', 'index', 'trigger')
           AND name NOT LIKE 'sqlite_%'
         ORDER BY type, name",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
            row.get::<_, String>(2)?,
        ))
    })?;

    rows.collect()
}

fn main() -> Result<()> {
    let source_path = "full-schema-source.db";
    let encrypted_path = "full-schema-encrypted.db";
    let password = "laboratory-full-schema-password";

    // Не перезаписываем существующие лабораторные базы.
    if Path::new(source_path).exists() || Path::new(encrypted_path).exists() {
        println!("Тестовые файлы уже существуют. Ничего не изменено.");
        return Ok(());
    }

    // 1. Создаём базу с полной схемой шести миграций Maybe Later.
    let source = Connection::open(source_path)?;

    source.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS app_metadata (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );

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

        CREATE TABLE IF NOT EXISTS tags (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            color TEXT NOT NULL
        );

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
            WHERE EXISTS (
                SELECT 1 FROM json_each(entries.tags)
                WHERE value = OLD.name
            );
        END;

        CREATE TRIGGER IF NOT EXISTS trg_tag_delete
        AFTER DELETE ON tags
        BEGIN
            UPDATE entries
            SET tags = (
                SELECT json_group_array(value)
                FROM (
                    SELECT value FROM json_each(entries.tags)
                    WHERE value <> OLD.name
                    ORDER BY CAST(key AS INTEGER)
                )
            ), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
            WHERE EXISTS (
                SELECT 1 FROM json_each(entries.tags)
                WHERE value = OLD.name
            );
        END;

        CREATE TABLE IF NOT EXISTS preferences (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS calendar_colors (
            date TEXT PRIMARY KEY NOT NULL,
            color TEXT NOT NULL
        );

        INSERT INTO app_metadata
        VALUES ('calendar_colors.sqliteWriter.v1', '1');

        INSERT INTO entries
        VALUES (
            'entry-1',
            'Тестовая запись',
            '<p>Текст дневника</p>',
            '2026-10-22',
            '["Важное","Личное"]',
            '2026-10-22T10:00:00Z',
            '2026-10-22T10:00:00Z'
        );

        INSERT INTO tags
        VALUES ('tag-1', 'Важное', '#55AA88');

        INSERT INTO tags
        VALUES ('tag-2', 'Личное', '#AA5588');

        INSERT INTO preferences
        VALUES ('preferences.v1', '{"theme":"dark"}');

        INSERT INTO calendar_colors
        VALUES ('2026-10-22', '#55AA88');
        "#,
    )?;

    println!("1. Создана тестовая база с полной схемой Maybe Later.");

    let original_schema = schema(&source)?;
    assert_eq!(original_schema.len(), 8);

    drop(source);

    // 2. Создаём зашифрованную копию.
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
    drop(source);

    println!("2. Создана зашифрованная копия.");

    // 3. Открываем копию с правильным ключом.
    let encrypted = Connection::open(encrypted_path)?;
    encrypted.pragma_update(None, "key", password)?;

    // 4. Проверяем все таблицы, индекс и триггеры.
    let encrypted_schema = schema(&encrypted)?;
    assert_eq!(original_schema, encrypted_schema);

    println!("3. Все 5 таблиц, индекс и 2 триггера сохранены.");

    // 5. Проверяем данные.
    let content: String = encrypted.query_row(
        "SELECT content FROM entries WHERE id = 'entry-1'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(content, "<p>Текст дневника</p>");

    let metadata: String = encrypted.query_row(
        "SELECT value FROM app_metadata
         WHERE key = 'calendar_colors.sqliteWriter.v1'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(metadata, "1");

    let preferences: String = encrypted.query_row(
        "SELECT value FROM preferences WHERE key = 'preferences.v1'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(preferences, r#"{"theme":"dark"}"#);

    let color: String = encrypted.query_row(
        "SELECT color FROM calendar_colors WHERE date = '2026-10-22'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(color, "#55AA88");

    println!("4. Данные дневника, настройки и календарь сохранены.");

    // 6. Проверяем работу триггера переименования.
    encrypted.execute(
        "UPDATE tags SET name = 'Новое имя' WHERE id = 'tag-1'",
        [],
    )?;

    let renamed_tags: String = encrypted.query_row(
        "SELECT tags FROM entries WHERE id = 'entry-1'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(renamed_tags, r#"["Новое имя","Личное"]"#);

    println!("5. Триггер переименования тега работает.");

    // 7. Проверяем работу триггера удаления.
    encrypted.execute(
        "DELETE FROM tags WHERE id = 'tag-2'",
        [],
    )?;

    let remaining_tags: String = encrypted.query_row(
        "SELECT tags FROM entries WHERE id = 'entry-1'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(remaining_tags, r#"["Новое имя"]"#);

    println!("6. Триггер удаления тега работает.");

    drop(encrypted);

    // 8. Проверяем, что исходная база осталась неизменной.
    let original = Connection::open(source_path)?;

    assert_eq!(schema(&original)?, original_schema);

    let original_tags: String = original.query_row(
        "SELECT tags FROM entries WHERE id = 'entry-1'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(original_tags, r#"["Важное","Личное"]"#);

    println!("7. Исходная база не изменилась.");
    println!("Полный тест схемы завершён успешно!");

    Ok(())
}
