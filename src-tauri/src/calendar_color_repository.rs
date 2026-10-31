use rusqlite::Connection;

pub fn save_calendar_color(
    conn: &Connection,
    date: &str,
    color: Option<&str>,
) -> Result<(), String> {
    if !is_valid_date_format(date) {
        return Err("Некорректная дата календаря".to_string());
    }

    match color {
        None => {
            conn.execute("DELETE FROM calendar_colors WHERE date = ?1", [date])
                .map_err(|e| e.to_string())?;
        }
        Some(value) => {
            if value.is_empty() {
                return Err("Некорректный цвет календаря".to_string());
            }

            conn.execute(
                "INSERT INTO calendar_colors (date, color) VALUES (?1, ?2) \
                 ON CONFLICT(date) DO UPDATE SET color = excluded.color",
                (date, value),
            )
            .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn is_valid_date_format(date: &str) -> bool {
    let bytes = date.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_updates_calendar_color() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE calendar_colors (
                date TEXT PRIMARY KEY NOT NULL,
                color TEXT NOT NULL
            );",
        )
        .unwrap();

        save_calendar_color(&conn, "2026-10-30", Some("#ff0000")).unwrap();
        save_calendar_color(&conn, "2026-10-30", Some("#00ff00")).unwrap();

        let (color, count): (String, i64) = (
            conn.query_row(
                "SELECT color FROM calendar_colors WHERE date = ?1",
                ["2026-10-30"],
                |row| row.get(0),
            )
            .unwrap(),
            conn.query_row(
                "SELECT COUNT(*) FROM calendar_colors",
                [],
                |row| row.get(0),
            )
            .unwrap(),
        );

        assert_eq!(color, "#00ff00");
        assert_eq!(count, 1);
    }
}

#[cfg(test)]
mod delete_tests {
    use super::*;

    #[test]
    fn deleting_calendar_color_removes_saved_record() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE calendar_colors (
                date TEXT PRIMARY KEY NOT NULL,
                color TEXT NOT NULL
            );",
        )
        .unwrap();

        save_calendar_color(&conn, "2026-10-30", Some("#ff0000")).unwrap();
        save_calendar_color(&conn, "2026-10-30", None).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar_colors WHERE date = ?1",
                ["2026-10-30"],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(count, 0);
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn rejects_invalid_date_and_empty_color() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE calendar_colors (
                date TEXT PRIMARY KEY NOT NULL,
                color TEXT NOT NULL
            );",
        )
        .unwrap();

        assert!(save_calendar_color(&conn, "30-10-2026", Some("#ff0000")).is_err());
        assert!(save_calendar_color(&conn, "2026-10-30", Some("")).is_err());

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM calendar_colors", [], |row| row.get(0))
            .unwrap();

        assert_eq!(count, 0);
    }
}

const SQLITE_HANDOVER_KEY: &str = "calendar_colors.sqliteWriter.v1";

pub fn migrate_calendar_colors(
    conn: &mut Connection,
    legacy_colors: &[(String, String)],
    browser_handed_over: bool,
) -> Result<Vec<(String, String)>, String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let marker_exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM app_metadata WHERE key = ?1)",
            [SQLITE_HANDOVER_KEY],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    let colors_exist: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM calendar_colors)",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if !marker_exists && !colors_exist && !browser_handed_over {
        for (date, color) in legacy_colors {
            if !is_valid_date_format(date) || color.is_empty() {
                return Err("Некорректные дата или цвет в localStorage календаря".to_string());
            }

            tx.execute(
                "INSERT INTO calendar_colors (date, color) VALUES (?1, ?2)",
                (date, color),
            )
            .map_err(|e| e.to_string())?;
        }
    }

    tx.execute(
        "INSERT INTO app_metadata (key, value) VALUES (?1, '1') \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [SQLITE_HANDOVER_KEY],
    )
    .map_err(|e| e.to_string())?;

    let colors = {
        let mut stmt = tx
            .prepare("SELECT date, color FROM calendar_colors ORDER BY date")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<(String, String)>, _>>()
            .map_err(|e| e.to_string())?
    };

    tx.commit().map_err(|e| e.to_string())?;
    Ok(colors)
}

#[cfg(test)]
mod migration_existing_tests {
    use super::*;

    #[test]
    fn migration_preserves_existing_sqlite_colors() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE calendar_colors (
                date TEXT PRIMARY KEY NOT NULL,
                color TEXT NOT NULL
            );
            CREATE TABLE app_metadata (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );
            INSERT INTO calendar_colors (date, color)
            VALUES ('2026-10-30', '#00ff00');",
        )
        .unwrap();

        let legacy = vec![("2026-10-30".to_string(), "#ff0000".to_string())];
        let colors = migrate_calendar_colors(&mut conn, &legacy, false).unwrap();

        assert_eq!(colors, vec![("2026-10-30".to_string(), "#00ff00".to_string())]);

        let marker: String = conn
            .query_row(
                "SELECT value FROM app_metadata WHERE key = ?1",
                [SQLITE_HANDOVER_KEY],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(marker, "1");
    }
}

#[cfg(test)]
mod migration_rollback_tests {
    use super::*;

    #[test]
    fn migration_rolls_back_on_invalid_legacy_color() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE calendar_colors (
                date TEXT PRIMARY KEY NOT NULL,
                color TEXT NOT NULL
            );
            CREATE TABLE app_metadata (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );",
        )
        .unwrap();

        let legacy = vec![
            ("2026-10-30".to_string(), "#00ff00".to_string()),
            ("invalid-date".to_string(), "#ff0000".to_string()),
        ];

        assert!(migrate_calendar_colors(&mut conn, &legacy, false).is_err());

        let color_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM calendar_colors", [], |row| row.get(0))
            .unwrap();

        let marker_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM app_metadata", [], |row| row.get(0))
            .unwrap();

        assert_eq!(color_count, 0);
        assert_eq!(marker_count, 0);
    }
}

#[cfg(test)]
mod migration_import_tests {
    use super::*;

    #[test]
    fn migration_imports_legacy_colors_once() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE calendar_colors (
                date TEXT PRIMARY KEY NOT NULL,
                color TEXT NOT NULL
            );
            CREATE TABLE app_metadata (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );",
        )
        .unwrap();

        let legacy = vec![
            ("2026-10-30".to_string(), "#ff0000".to_string()),
            ("2026-10-31".to_string(), "#00ff00".to_string()),
        ];

        let colors = migrate_calendar_colors(&mut conn, &legacy, false).unwrap();
        assert_eq!(colors, legacy);

        let changed_legacy = vec![
            ("2026-10-30".to_string(), "#0000ff".to_string()),
        ];

        let colors_again =
            migrate_calendar_colors(&mut conn, &changed_legacy, false).unwrap();

        assert_eq!(colors_again, legacy);

        let marker: String = conn
            .query_row(
                "SELECT value FROM app_metadata WHERE key = ?1",
                [SQLITE_HANDOVER_KEY],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(marker, "1");
    }
}

pub fn needs_legacy_calendar_color_import(conn: &Connection) -> Result<bool, String> {
    let marker_exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM app_metadata WHERE key = ?1)",
            [SQLITE_HANDOVER_KEY],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    let colors_exist: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM calendar_colors)",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    Ok(!marker_exists && !colors_exist)
}

pub fn load_calendar_colors(conn: &Connection) -> Result<Vec<(String, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT date, color FROM calendar_colors ORDER BY date")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<(String, String)>, _>>()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod load_colors_tests {
    use super::*;

    #[test]
    fn loads_calendar_colors_in_date_order() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE calendar_colors (
                date TEXT PRIMARY KEY NOT NULL,
                color TEXT NOT NULL
            );
            INSERT INTO calendar_colors (date, color)
            VALUES ('2026-10-31', '#00ff00');
            INSERT INTO calendar_colors (date, color)
            VALUES ('2026-10-30', '#ff0000');",
        )
        .unwrap();

        let colors = load_calendar_colors(&conn).unwrap();

        assert_eq!(
            colors,
            vec![
                ("2026-10-30".to_string(), "#ff0000".to_string()),
                ("2026-10-31".to_string(), "#00ff00".to_string()),
            ]
        );
    }
}
