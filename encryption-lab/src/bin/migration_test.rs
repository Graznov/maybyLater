
use rusqlite::{Connection, Result};
use std::path::Path;

fn main() -> Result<()> {
    let source_path = "migration-source.db";
    let encrypted_path = "migration-encrypted.db";
    let password = "laboratory-migration-password";

    // Не перезаписываем результаты предыдущих тестов.
    if Path::new(source_path).exists() || Path::new(encrypted_path).exists() {
        println!("Тестовые файлы уже существуют. Ничего не изменено.");
        return Ok(());
    }

    // 1. Создаём обычную SQLite-базу с тестовыми данными.
    let source = Connection::open(source_path)?;

    source.execute_batch(
        "
        CREATE TABLE entries (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            content TEXT NOT NULL
        );

        CREATE TABLE tags (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL
        );

        CREATE TABLE calendar_colors (
            date TEXT PRIMARY KEY,
            color TEXT NOT NULL
        );

        INSERT INTO entries VALUES (
            'entry-1',
            'Тестовая запись',
            'Текст для проверки миграции'
        );

        INSERT INTO tags VALUES ('tag-1', 'Важное');
        INSERT INTO calendar_colors VALUES ('2026-10-22', '#55AA88');
        ",
    )?;

    println!("1. Создана обычная SQLite-база с тестовыми данными.");

    // Закрываем исходную базу перед переносом.
    drop(source);

    // 2. Открываем исходную базу и подключаем новую зашифрованную.
    let source = Connection::open(source_path)?;

    source.execute(
        "ATTACH DATABASE ?1 AS encrypted KEY ?2",
        rusqlite::params![encrypted_path, password],
    )?;

    // SQLCipher копирует таблицы и данные в зашифрованную базу.
    source.query_row(
        "SELECT sqlcipher_export('encrypted')",
        [],
        |_row| Ok(()),
    )?;

    source.execute_batch("DETACH DATABASE encrypted;")?;
    drop(source);

    println!("2. Создана зашифрованная копия базы.");

    // 3. Проверяем содержимое новой базы с правильным ключом.
    let encrypted = Connection::open(encrypted_path)?;
    encrypted.pragma_update(None, "key", password)?;

    let entry: (String, String) = encrypted.query_row(
        "SELECT title, content FROM entries WHERE id = 'entry-1'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let tag: String = encrypted.query_row(
        "SELECT name FROM tags WHERE id = 'tag-1'",
        [],
        |row| row.get(0),
    )?;

    let color: String = encrypted.query_row(
        "SELECT color FROM calendar_colors WHERE date = '2026-10-22'",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(entry.0, "Тестовая запись");
    assert_eq!(entry.1, "Текст для проверки миграции");
    assert_eq!(tag, "Важное");
    assert_eq!(color, "#55AA88");

    println!("3. Записи, теги и календарные отметки совпадают.");

    drop(encrypted);

    // 4. Убеждаемся, что исходная база осталась обычной и читаемой.
    let original = Connection::open(source_path)?;

    let original_content: String = original.query_row(
        "SELECT content FROM entries WHERE id = 'entry-1'",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(original_content, "Текст для проверки миграции");

    println!("4. Исходная база сохранилась и читается без ключа.");
    println!("Тест миграции завершён успешно!");

    Ok(())
}
