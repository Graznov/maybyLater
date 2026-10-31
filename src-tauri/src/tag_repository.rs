use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
}

pub fn load_tags(conn: &Connection) -> Result<Vec<Tag>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, color FROM tags ORDER BY rowid")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut tags = Vec::new();
    let mut ids = std::collections::HashSet::new();

    for row in rows {
        let tag = row.map_err(|e| e.to_string())?;

        if tag.id.is_empty()
            || tag.name.is_empty()
            || tag.color.is_empty()
            || !ids.insert(tag.id.clone())
        {
            return Err("Некорректный справочник тегов SQLite".into());
        }

        tags.push(tag);
    }

    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE tags (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                color TEXT NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn load_tags_preserves_insertion_order() {
        let conn = test_db();
        conn.execute(
            "INSERT INTO tags (id, name, color) VALUES (?1, ?2, ?3)",
            ("tag-b", "Второй", "#222222"),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tags (id, name, color) VALUES (?1, ?2, ?3)",
            ("tag-a", "Первый", "#111111"),
        )
        .unwrap();

        let tags = load_tags(&conn).unwrap();

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].id, "tag-b");
        assert_eq!(tags[1].id, "tag-a");
    }

    #[test]
    fn load_tags_rejects_empty_fields() {
        let conn = test_db();
        conn.execute(
            "INSERT INTO tags (id, name, color) VALUES (?1, ?2, ?3)",
            ("tag-1", "", "#111111"),
        )
        .unwrap();

        assert_eq!(
            load_tags(&conn).unwrap_err(),
            "Некорректный справочник тегов SQLite"
        );
    }
}

pub fn insert_tag(conn: &Connection, tag: &Tag) -> Result<(), String> {
    conn.execute(
        "INSERT INTO tags (id, name, color) VALUES (?1, ?2, ?3)",
        (&tag.id, &tag.name, &tag.color),
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod insert_tests {
    use super::*;

    #[test]
    fn insert_tag_creates_tag_and_rejects_duplicate_id() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE tags (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                color TEXT NOT NULL
            );",
        )
        .unwrap();

        let tag = Tag {
            id: "tag-1".into(),
            name: "Работа".into(),
            color: "#123456".into(),
        };

        insert_tag(&conn, &tag).unwrap();

        assert_eq!(load_tags(&conn).unwrap(), vec![tag.clone()]);
        assert!(insert_tag(&conn, &tag).is_err());
        assert_eq!(load_tags(&conn).unwrap(), vec![tag]);
    }
}

pub fn update_tag(conn: &Connection, tag: &Tag) -> Result<(), String> {
    let affected = conn
        .execute(
            "UPDATE tags SET name = ?1, color = ?2 WHERE id = ?3",
            (&tag.name, &tag.color, &tag.id),
        )
        .map_err(|e| e.to_string())?;

    if affected != 1 {
        return Err("Тег не найден в SQLite".into());
    }

    Ok(())
}

#[cfg(test)]
mod update_tests {
    use super::*;

    #[test]
    fn update_tag_renames_entry_tags_via_trigger() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE tags (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                color TEXT NOT NULL
            );
            CREATE TABLE entries (
                id TEXT PRIMARY KEY NOT NULL,
                tags TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TRIGGER trg_tag_rename
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
            INSERT INTO tags VALUES ('tag-1', 'Работа', '#111111');
            INSERT INTO entries VALUES (
                'entry-1', '["Личное","Работа","Идеи"]', 'original'
            );
            "#,
        )
        .unwrap();

        let updated = Tag {
            id: "tag-1".into(),
            name: "Проекты".into(),
            color: "#222222".into(),
        };

        update_tag(&conn, &updated).unwrap();

        assert_eq!(load_tags(&conn).unwrap(), vec![updated]);

        let (tags, updated_at): (String, String) = conn
            .query_row(
                "SELECT tags, updated_at FROM entries WHERE id = 'entry-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(tags, r#"["Личное","Проекты","Идеи"]"#);
        assert_ne!(updated_at, "original");
    }
}

pub fn delete_tag(conn: &Connection, id: &str) -> Result<(), String> {
    let affected = conn
        .execute("DELETE FROM tags WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;

    if affected != 1 {
        return Err("Тег не найден в SQLite".into());
    }

    Ok(())
}

#[cfg(test)]
mod delete_tests {
    use super::*;

    #[test]
    fn delete_tag_removes_entry_tags_via_trigger() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE tags (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                color TEXT NOT NULL
            );
            CREATE TABLE entries (
                id TEXT PRIMARY KEY NOT NULL,
                tags TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TRIGGER trg_tag_delete
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
            INSERT INTO tags VALUES ('tag-1', 'Работа', '#111111');
            INSERT INTO entries VALUES (
                'entry-1', '["Личное","Работа","Идеи"]', 'original'
            );
            INSERT INTO entries VALUES (
                'entry-2', '["Личное"]', 'unchanged'
            );
            "#,
        )
        .unwrap();

        delete_tag(&conn, "tag-1").unwrap();

        assert!(load_tags(&conn).unwrap().is_empty());

        let (tags, updated_at): (String, String) = conn
            .query_row(
                "SELECT tags, updated_at FROM entries WHERE id = 'entry-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(tags, r#"["Личное","Идеи"]"#);
        assert_ne!(updated_at, "original");

        let (other_tags, other_updated_at): (String, String) = conn
            .query_row(
                "SELECT tags, updated_at FROM entries WHERE id = 'entry-2'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(other_tags, r#"["Личное"]"#);
        assert_eq!(other_updated_at, "unchanged");
        assert!(delete_tag(&conn, "tag-1").is_err());
    }
}
