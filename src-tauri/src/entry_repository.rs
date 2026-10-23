use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub content: String,
    pub date: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Сохраняет запись через переданное соединение SQLite.
///
/// Функция сама не открывает файл базы данных.
/// Позже соединение будет предоставлять защищённый Rust-слой.
pub fn insert_entry(
    conn: &Connection,
    entry: &Entry,
) -> Result<(), String> {
    let tags = serde_json::to_string(&entry.tags)
        .map_err(|e| e.to_string())?;

    let affected = conn
        .execute(
            "INSERT INTO entries (
                id, title, content, date, tags, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                entry.id,
                entry.title,
                entry.content,
                entry.date,
                tags,
                entry.created_at,
                entry.updated_at
            ],
        )
        .map_err(|e| e.to_string())?;

    if affected != 1 {
        return Err(format!("INSERT affected {affected} rows"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();

        conn.execute_batch(
            "
            CREATE TABLE entries (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                date TEXT NOT NULL,
                tags TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            ",
        )
        .unwrap();

        conn
    }

    fn sample_entry() -> Entry {
        Entry {
            id: "test-entry-1".to_string(),
            title: "Тестовая запись".to_string(),
            content: "Искусственный текст".to_string(),
            date: "2026-01-01".to_string(),
            tags: vec!["тест".to_string()],
            created_at: "2026-01-01T10:00:00Z".to_string(),
            updated_at: "2026-01-01T10:00:00Z".to_string(),
        }
    }

    #[test]
    fn inserts_entry_with_json_tags() {
        let conn = test_db();
        let entry = sample_entry();

        insert_entry(&conn, &entry).unwrap();

        let (title, content, tags): (String, String, String) = conn
            .query_row(
                "SELECT title, content, tags FROM entries WHERE id = ?1",
                [&entry.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(title, "Тестовая запись");
        assert_eq!(content, "Искусственный текст");
        assert_eq!(tags, r#"["тест"]"#);
    }

    #[test]
    fn duplicate_id_returns_error_without_overwriting() {
        let conn = test_db();
        let entry = sample_entry();

        insert_entry(&conn, &entry).unwrap();

        let mut duplicate = sample_entry();
        duplicate.title = "Другая запись".to_string();

        assert!(insert_entry(&conn, &duplicate).is_err());

        let title: String = conn
            .query_row(
                "SELECT title FROM entries WHERE id = ?1",
                [&entry.id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(title, "Тестовая запись");
    }
}