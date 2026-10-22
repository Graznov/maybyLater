
use rusqlite::{Connection, OpenFlags, Result};
use std::path::Path;

fn main() -> Result<()> {
    let db_path = "test-encrypted.db";
    let password = "laboratory-test-password";

    // Не перезаписываем результаты предыдущего теста.
    if Path::new(db_path).exists() {
        println!("Тестовая база уже существует. Удали её перед повторным запуском.");
        return Ok(());
    }

    // 1. Создаём базу с ключом шифрования.
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "key", password)?;

    conn.execute(
        "CREATE TABLE diary (id INTEGER PRIMARY KEY, content TEXT NOT NULL)",
        [],
    )?;

    conn.execute(
        "INSERT INTO diary (content) VALUES (?1)",
        ["Это секретная тестовая запись Maybe Later"],
    )?;

    println!("1. Тестовая база создана, запись сохранена.");

    drop(conn);

    // 2. Проверяем, что правильный ключ открывает базу.
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "key", password)?;

    let content: String =
        conn.query_row("SELECT content FROM diary WHERE id = 1", [], |row| row.get(0))?;

    println!("2. С правильным ключом прочитано: {content}");

    drop(conn);

    // 3. Проверяем открытие без ключа.
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;

    match conn.query_row("SELECT content FROM diary WHERE id = 1", [], |row| {
        row.get::<_, String>(0)
    }) {
        Ok(_) => println!("ОШИБКА: база открылась без ключа!"),
        Err(_) => println!("3. Без ключа прочитать базу не удалось — отлично!"),
    }

    drop(conn);

    // 4. Проверяем неправильный ключ.
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.pragma_update(None, "key", "wrong-password")?;

    match conn.query_row("SELECT content FROM diary WHERE id = 1", [], |row| {
        row.get::<_, String>(0)
    }) {
        Ok(_) => println!("ОШИБКА: база открылась с неправильным ключом!"),
        Err(_) => println!("4. С неправильным ключом прочитать базу не удалось — отлично!"),
    }

    println!("Тест завершён.");

    Ok(())
}
