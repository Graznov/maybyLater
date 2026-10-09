import Database from '@tauri-apps/plugin-sql';

export async function testSqliteConnection(): Promise<void> {
  try {
    const db = await Database.load('sqlite:maybe-later.db');
    try {
      const tables = await db.select<{ name: string }[]>(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'app_metadata'"
      );
      if (tables.length === 1) {
        console.log('✅ SQLite: база подключена, таблица app_metadata найдена');
      } else {
        console.error('❌ SQLite: таблица app_metadata не найдена');
      }
    } finally {
      await db.close();
    }
  } catch (error) {
    console.error('❌ SQLite: ошибка подключения', error);
  }
}
