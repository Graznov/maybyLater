import Database from '@tauri-apps/plugin-sql';

type TableNameRow = { name: string };
type TestEntryRow = {
  id: string;
  title: string;
  content: string;
  date: string;
  tags: string;
  created_at: string;
  updated_at: string;
};

/** Temporary diagnostic only. Does not access the diary's localStorage data. */
export async function testSqliteConnection(): Promise<void> {
  let db: Awaited<ReturnType<typeof Database.load>> | undefined;
  let testId: string | undefined;

  try {
    db = await Database.load('sqlite:maybe-later.db');

    const tables = await db.select<TableNameRow[]>(
      "SELECT name FROM sqlite_master WHERE type = 'table' AND name IN ('app_metadata', 'entries')"
    );
    const names = new Set(tables.map((table) => table.name));
    if (!names.has('app_metadata') || !names.has('entries')) {
      throw new Error('Отсутствует таблица app_metadata или entries');
    }
    console.log('✅ SQLite: обе таблицы найдены');

    // The prefix and random UUID ensure this test never targets a real entry.
    testId = `__sqlite_test__${crypto.randomUUID()}`;
    const now = new Date().toISOString();
    const tags = ['sqlite-test'];

    const inserted = await db.execute(
      `INSERT INTO entries (id, title, content, date, tags, created_at, updated_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7)`,
      [testId, 'SQLite test entry', '<p>Test content</p>', '2026-01-01', JSON.stringify(tags), now, now]
    );
    if (inserted.rowsAffected !== 1) throw new Error('INSERT не добавил одну запись');
    console.log('✅ SQLite: тестовая запись создана');

    const rows = await db.select<TestEntryRow[]>(
      'SELECT * FROM entries WHERE id = $1',
      [testId]
    );
    if (
      rows.length !== 1 ||
      rows[0].title !== 'SQLite test entry' ||
      rows[0].content !== '<p>Test content</p>' ||
      rows[0].date !== '2026-01-01' ||
      JSON.stringify(JSON.parse(rows[0].tags)) !== JSON.stringify(tags) ||
      rows[0].created_at !== now
    ) {
      throw new Error('SELECT вернул неожиданные данные');
    }
    console.log('✅ SQLite: запись прочитана, поля совпадают');

    const updated = await db.execute(
      'UPDATE entries SET title = $1, updated_at = $2 WHERE id = $3',
      ['SQLite updated entry', new Date().toISOString(), testId]
    );
    if (updated.rowsAffected !== 1) throw new Error('UPDATE не обновил одну запись');
    const changed = await db.select<{ title: string }[]>(
      'SELECT title FROM entries WHERE id = $1', [testId]
    );
    if (changed.length !== 1 || changed[0].title !== 'SQLite updated entry') {
      throw new Error('UPDATE не подтвердился чтением');
    }
    console.log('✅ SQLite: запись обновлена');
  } catch (error) {
    console.error('❌ SQLite: ошибка проверки', error);
  } finally {
    if (db) {
      try {
        if (testId) {
          await db.execute('DELETE FROM entries WHERE id = $1', [testId]);
          const remaining = await db.select<{ id: string }[]>(
            'SELECT id FROM entries WHERE id = $1', [testId]
          );
          if (remaining.length !== 0) throw new Error('Тестовая запись не удалена');
          console.log('✅ SQLite: тестовая запись удалена');
        }
      } catch (error) {
        console.error('❌ SQLite: ошибка очистки тестовой записи', error);
      } finally {
        await db.close();
      }
    }
  }
}
