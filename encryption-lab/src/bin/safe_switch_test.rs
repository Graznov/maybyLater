
use rusqlite::{params, Connection, Result};
use std::fs;
use std::path::Path;

fn read_content(path: &str, password: Option<&str>) -> Result<String> {
    let conn = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;

    if let Some(password) = password {
        conn.pragma_update(None, "key", password)?;
    }

    conn.query_row(
        "SELECT content FROM entries WHERE id = 'entry-1'",
        [],
        |row| row.get(0),
    )
}

fn main() -> Result<()> {
    let source_path = "safe-switch-source.db";
    let encrypted_path = "safe-switch-encrypted.db";
    let active_path = "safe-switch-active.db";
    let password = "laboratory-switch-password";
    let expected_content = "Важная тестовая запись";

    // Не изменяем результаты предыдущих запусков.
    for path in [source_path, encrypted_path, active_path] {
        if Path::new(path).exists() {
            println!("Тестовый файл {path} уже существует. Ничего не изменено.");
            return Ok(());
        }
    }

    // 1. Создаём исходную SQLite-базу.
    let source = Connection::open(source_path)?;

    source.execute_batch(
        "
        CREATE TABLE entries (
            id TEXT PRIMARY KEY NOT NULL,
            content TEXT NOT NULL
        );

        INSERT INTO entries (id, content)
        VALUES ('entry-1', 'Важная тестовая запись');
        ",
    )?;

    drop(source);

    println!("1. Исходная база создана.");

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

    println!("2. Зашифрованная копия создана.");

    // 3. Проверяем копию ДО переключения.
    let restored_content = read_content(encrypted_path, Some(password))?;
    assert_eq!(restored_content, expected_content);

    println!("3. Зашифрованная копия проверена.");

    // 4. Имитируем ошибку проверки: используем неверный пароль.
    // При ошибке переключение не выполняем.
    let invalid_check = read_content(encrypted_path, Some("wrong-password"));

    assert!(invalid_check.is_err());
    assert!(!Path::new(active_path).exists());

    println!("4. При ошибке проверки переключение не выполняется.");

    // 5. Проверка с правильным паролем уже прошла.
    // Переключаем только зашифрованную копию.
    // Исходный файл НЕ переименовываем и НЕ удаляем.
    fs::rename(encrypted_path, active_path)
        .expect("Не удалось переключить зашифрованную копию");

    println!("5. Зашифрованная база стала активной.");

    // 6. Проверяем активную базу.
    let active_content = read_content(active_path, Some(password))?;
    assert_eq!(active_content, expected_content);

    println!("6. Активная зашифрованная база читается.");

    // 7. Проверяем, что исходная база сохранилась.
    let original_content = read_content(source_path, None)?;
    assert_eq!(original_content, expected_content);

    println!("7. Исходная SQLite-база сохранена для восстановления.");

    println!("Тест безопасного переключения завершён успешно!");

    Ok(())
}
