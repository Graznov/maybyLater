
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

/// Загружает записи через переданное SQLite-соединение.
///
/// Не открывает файл базы данных и сохраняет порядок сортировки frontend.
pub fn load_entries(conn: &Connection) -> Result<Vec<Entry>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, content, date, tags, created_at, updated_at
             FROM entries
             ORDER BY date DESC, created_at DESC, id",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut entries = Vec::new();
    let mut ids = std::collections::HashSet::new();

    for row in rows {
        let (id, title, content, date, raw_tags, created_at, updated_at) =
            row.map_err(|e| e.to_string())?;

        let tags: Vec<String> = serde_json::from_str(&raw_tags)
            .map_err(|e| format!("Неверный формат тегов записи {id}: {e}"))?;

        if id.is_empty() || !ids.insert(id.clone()) {
            return Err(format!("Некорректный или повторяющийся ID: {id}"));
        }

        entries.push(Entry {
            id,
            title,
            content,
            date,
            tags,
            created_at,
            updated_at,
        });
    }

    Ok(entries)
}

/// Обновляет существующую запись через переданное SQLite-соединение.
///
/// Не изменяет id и created_at.
/// Возвращает ошибку, если обновлена не ровно одна запись.
pub fn update_entry(
    conn: &Connection,
    entry: &Entry,
) -> Result<(), String> {
    let tags = serde_json::to_string(&entry.tags)
        .map_err(|e| e.to_string())?;

    let affected = conn
        .execute(
            "UPDATE entries
             SET title = ?1,
                 content = ?2,
                 date = ?3,
                 tags = ?4,
                 updated_at = ?5
             WHERE id = ?6",
            params![
                entry.title,
                entry.content,
                entry.date,
                tags,
                entry.updated_at,
                entry.id
            ],
        )
        .map_err(|e| e.to_string())?;

    if affected != 1 {
        return Err(format!(
            "UPDATE affected {affected} rows for {}",
            entry.id
        ));
    }

    Ok(())
}

/// Удаляет ровно одну существующую запись через переданное соединение.
pub fn delete_entry(conn: &Connection, id: &str) -> Result<(), String> {
    let affected = conn.execute("DELETE FROM entries WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    if affected != 1 {
        return Err(format!("DELETE affected {affected} rows for {id}"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn test_db() -> Connection {
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

    #[test]
    fn load_entries_rejects_non_text_title() {
        let conn = test_db();
        conn.execute(
            "INSERT INTO entries (id, title, content, date, tags, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params!["invalid-title", vec![0xff_u8], "Текст", "2026-01-01", "[]", "2026-01-01T10:00:00Z", "2026-01-01T10:00:00Z"],
        ).unwrap();
        assert!(load_entries(&conn).is_err());
    }

    #[test]
    fn delete_entry_removes_existing_entry() {
        let conn = test_db();
        let entry = sample_entry();
        insert_entry(&conn, &entry).unwrap();

        delete_entry(&conn, &entry.id).unwrap();

        assert!(load_entries(&conn).unwrap().is_empty());
    }

    #[test]
    fn delete_entry_rejects_missing_entry() {
        let conn = test_db();
        let error = delete_entry(&conn, "missing-id").unwrap_err();

        assert!(error.contains("DELETE affected 0 rows"));
    }

    pub(super) fn sample_entry() -> Entry {
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

    #[test]
    fn loads_entries_in_expected_order() {
        let conn = test_db();

        let mut older = sample_entry();
        older.id = "older-entry".to_string();
        older.title = "Старая запись".to_string();
        older.date = "2026-01-01".to_string();

        let mut newer = sample_entry();
        newer.id = "newer-entry".to_string();
        newer.title = "Новая запись".to_string();
        newer.date = "2026-01-02".to_string();
        newer.tags = vec!["важное".to_string(), "личное".to_string()];

        // Вставляем старую запись первой, чтобы проверить SQL-сортировку.
        insert_entry(&conn, &older).unwrap();
        insert_entry(&conn, &newer).unwrap();

        let entries = load_entries(&conn).unwrap();

        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].id, "newer-entry");
        assert_eq!(entries[0].title, "Новая запись");
        assert_eq!(entries[0].tags, vec!["важное", "личное"]);

        assert_eq!(entries[1].id, "older-entry");
        assert_eq!(entries[1].title, "Старая запись");
        assert_eq!(entries[1].tags, vec!["тест"]);
    }

    #[test]
    fn load_entries_rejects_invalid_tags() {
        let conn = test_db();

        conn.execute(
            "INSERT INTO entries (
                id, title, content, date, tags, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "invalid-tags",
                "Тестовая запись",
                "Искусственный текст",
                "2026-01-01",
                "{invalid json}",
                "2026-01-01T10:00:00Z",
                "2026-01-01T10:00:00Z",
            ],
        )
        .unwrap();

        let result = load_entries(&conn);

        assert!(result.is_err());
    }

    #[test]
    fn update_entry_changes_fields_but_preserves_created_at() {
        let conn = test_db();
        let original = sample_entry();

        insert_entry(&conn, &original).unwrap();

        let mut updated = sample_entry();
        updated.title = "Обновлённый заголовок".to_string();
        updated.content = "Обновлённый текст".to_string();
        updated.date = "2026-01-02".to_string();
        updated.tags = vec!["новый тег".to_string()];
        updated.updated_at = "2026-01-02T12:00:00Z".to_string();

        // Даже если передать другое created_at, в базе оно не меняется.
        updated.created_at = "2099-01-01T00:00:00Z".to_string();

        update_entry(&conn, &updated).unwrap();

        let entries = load_entries(&conn).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, original.id);
        assert_eq!(entries[0].title, updated.title);
        assert_eq!(entries[0].content, updated.content);
        assert_eq!(entries[0].date, updated.date);
        assert_eq!(entries[0].tags, updated.tags);
        assert_eq!(entries[0].updated_at, updated.updated_at);
        assert_eq!(entries[0].created_at, original.created_at);
    }

    #[test]
    fn update_entry_rejects_missing_entry() {
        let conn = test_db();
        let entry = sample_entry();

        let result = update_entry(&conn, &entry);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "UPDATE affected 0 rows for test-entry-1"
        );

        let entries = load_entries(&conn).unwrap();
        assert!(entries.is_empty());
    }
}

#[cfg(test)]
mod change_tags_tests {
    use super::*;

    #[test]
    fn change_tags_rolls_back_when_an_entry_is_missing() {
        let mut conn = tests::test_db();

        let original = tests::sample_entry();
        insert_entry(&conn, &original).unwrap();

        let replacements = vec![
            (original.id.clone(), vec!["новый тег".to_string()]),
            ("missing-entry".to_string(), vec!["новый тег".to_string()]),
        ];

        let result = change_entry_tags(
            &mut conn,
            &replacements,
            "2026-01-02T12:00:00Z",
        );

        assert!(result.is_err());

        let entries = load_entries(&conn).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tags, original.tags);
        assert_eq!(entries[0].updated_at, original.updated_at);
    }
}

/// Atomically replace tags for the specified entries.
pub fn change_entry_tags(
    conn: &mut Connection,
    replacements: &[(String, Vec<String>)],
    updated_at: &str,
) -> Result<usize, String> {
    if replacements.is_empty() {
        return Ok(0);
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for (id, tags) in replacements {
        let tags_json = serde_json::to_string(tags).map_err(|e| e.to_string())?;

        let affected = tx
            .execute(
                "UPDATE entries SET tags = ?1, updated_at = ?2 WHERE id = ?3",
                params![tags_json, updated_at, id],
            )
            .map_err(|e| e.to_string())?;

        if affected != 1 {
            return Err(format!(
                "Изменено {affected} строк для записи {id}. Изменения отменены."
            ));
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(replacements.len())
}

#[cfg(test)]
mod change_tags_success_tests {
    use super::*;

    #[test]
    fn change_tags_updates_multiple_entries_atomically() {
        let mut conn = tests::test_db();

        let first = tests::sample_entry();
        let mut second = tests::sample_entry();
        second.id = "test-entry-2".to_string();

        insert_entry(&conn, &first).unwrap();
        insert_entry(&conn, &second).unwrap();

        let replacements = vec![
            (first.id.clone(), vec!["новый тег".to_string()]),
            (second.id.clone(), vec!["другой тег".to_string()]),
        ];
        let now = "2026-01-02T12:00:00Z";

        let changed = change_entry_tags(&mut conn, &replacements, now).unwrap();

        assert_eq!(changed, 2);

        let entries = load_entries(&conn).unwrap();
        assert_eq!(entries.len(), 2);

        for entry in &entries {
            let expected = replacements
                .iter()
                .find(|(id, _)| id == &entry.id)
                .unwrap();

            assert_eq!(&entry.tags, &expected.1);
            assert_eq!(entry.updated_at, now);
            assert_eq!(entry.created_at, first.created_at);
        }
    }
}
