
use rusqlite::{params, Connection, Result};
use std::path::Path;

fn main() -> Result<()> {
    let source_path = "base64-photo-source.db";
    let encrypted_path = "base64-photo-encrypted.db";
    let password = "laboratory-photo-password";

    // Не перезаписываем существующие лабораторные файлы.
    if Path::new(source_path).exists() || Path::new(encrypted_path).exists() {
        println!("Тестовые файлы уже существуют. Ничего не изменено.");
        return Ok(());
    }

    // Искусственная PNG-картинка 1x1 пиксель.
    let photo_base64 = concat!(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwC",
        "AAAAC0lEQVR42mP8/x8AAwMCAO+/lXcAAAAASUVORK5CYII="
    );

    // Такой формат HTML используется редактором дневника.
    let html = format!(
        "<p>Тестовая запись с фотографией</p>\
         <img src=\"data:image/png;base64,{photo_base64}\">"
    );

    // 1. Создаём обычную SQLite-базу.
    let source = Connection::open(source_path)?;

    source.execute_batch(
        "CREATE TABLE entries (
            id TEXT PRIMARY KEY NOT NULL,
            content TEXT NOT NULL
        );",
    )?;

    source.execute(
        "INSERT INTO entries (id, content) VALUES (?1, ?2)",
        params!["entry-photo-1", html],
    )?;

    println!("1. Создана тестовая запись с фотографией Base64.");

    drop(source);

    // 2. Создаём зашифрованную копию через SQLCipher.
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

    // 3. Открываем базу с правильным паролем.
    let encrypted = Connection::open(encrypted_path)?;
    encrypted.pragma_update(None, "key", password)?;

    let restored_html: String = encrypted.query_row(
        "SELECT content FROM entries WHERE id = 'entry-photo-1'",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(restored_html, html);
    assert!(restored_html.contains(photo_base64));

    println!("3. HTML и фотография Base64 восстановлены без изменений.");

    drop(encrypted);

    // 4. Проверяем, что без пароля база не читается.
    let without_password = Connection::open(encrypted_path)?;

    let no_key_result: Result<i64> = without_password.query_row(
        "SELECT COUNT(*) FROM entries",
        [],
        |row| row.get(0),
    );

    assert!(
        no_key_result.is_err(),
        "Зашифрованная база неожиданно открылась без пароля!"
    );

    println!("4. Без пароля прочитать зашифрованную базу нельзя.");

    drop(without_password);

    // 5. Проверяем неправильный пароль.
    let wrong_password = Connection::open(encrypted_path)?;
    wrong_password.pragma_update(None, "key", "wrong-password")?;

    let wrong_key_result: Result<i64> = wrong_password.query_row(
        "SELECT COUNT(*) FROM entries",
        [],
        |row| row.get(0),
    );

    assert!(
        wrong_key_result.is_err(),
        "Зашифрованная база неожиданно открылась с неверным паролем!"
    );

    println!("5. Неправильный пароль не позволяет прочитать базу.");

    drop(wrong_password);

    // 6. Проверяем, что исходная база не изменилась.
    let original = Connection::open(source_path)?;

    let original_html: String = original.query_row(
        "SELECT content FROM entries WHERE id = 'entry-photo-1'",
        [],
        |row| row.get(0),
    )?;

    assert_eq!(original_html, html);

    println!("6. Исходная база осталась неизменной.");
    println!("Тест шифрования фотографии Base64 завершён успешно!");

    Ok(())
}
