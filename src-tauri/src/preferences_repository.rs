use rusqlite::{Connection, OptionalExtension};

const KEY: &str = "preferences.v1";

pub fn load_preferences(conn: &Connection) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM preferences WHERE key = ?1",
        [KEY],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn insert_initial_preferences(conn: &Connection, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO preferences (key, value) VALUES (?1, ?2)",
        (KEY, value),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn save_preferences(conn: &Connection, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO preferences (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (KEY, value),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_preferences_returns_none_or_saved_snapshot() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE preferences (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);",
        )
        .unwrap();

        assert_eq!(load_preferences(&conn).unwrap(), None);

        conn.execute(
            "INSERT INTO preferences (key, value) VALUES (?1, ?2)",
            (KEY, r#"{"theme":"dark"}"#),
        )
        .unwrap();

        assert_eq!(
            load_preferences(&conn).unwrap(),
            Some(r#"{"theme":"dark"}"#.to_string())
        );
    }
}

#[cfg(test)]
mod initial_insert_tests {
    use super::*;

    #[test]
    fn initial_insert_does_not_overwrite_existing_preferences() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE preferences (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);",
        )
        .unwrap();

        insert_initial_preferences(&conn, r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(
            load_preferences(&conn).unwrap().as_deref(),
            Some(r#"{"theme":"dark"}"#)
        );

        insert_initial_preferences(&conn, r#"{"theme":"light"}"#).unwrap();
        assert_eq!(
            load_preferences(&conn).unwrap().as_deref(),
            Some(r#"{"theme":"dark"}"#)
        );
    }
}

#[cfg(test)]
mod save_tests {
    use super::*;

    #[test]
    fn save_preferences_inserts_and_updates_snapshot() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE preferences (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);",
        )
        .unwrap();

        save_preferences(&conn, r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(
            load_preferences(&conn).unwrap().as_deref(),
            Some(r#"{"theme":"dark"}"#)
        );

        save_preferences(&conn, r#"{"theme":"light"}"#).unwrap();
        assert_eq!(
            load_preferences(&conn).unwrap().as_deref(),
            Some(r#"{"theme":"light"}"#)
        );
    }
}
