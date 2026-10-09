import Database from '@tauri-apps/plugin-sql';
import type { Entry } from '../types/entry';

type EntryRow = {
  id: string;
  title: string;
  content: string;
  date: string;
  tags: string;
  created_at: string;
  updated_at: string;
};

function decodeRow(row: EntryRow): Entry {
  const tags: unknown = JSON.parse(row.tags);
  if (!Array.isArray(tags) || !tags.every(tag => typeof tag === 'string')) {
    throw new Error(`Неверный формат тегов у записи ${row.id}`);
  }
  const fields = [row.id, row.title, row.content, row.date, row.created_at, row.updated_at];
  if (!fields.every(field => typeof field === 'string') || row.id.length === 0) {
    throw new Error(`Неверный формат полей у записи ${String(row.id)}`);
  }
  return {
    id: row.id,
    title: row.title,
    content: row.content,
    date: row.date,
    tags,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

/** Read-only diagnostic: reconstruct Entry objects from SQLite, without using localStorage. */
export async function testReadVerification(): Promise<void> {
  let db: Awaited<ReturnType<typeof Database.load>> | undefined;
  try {
    db = await Database.load('sqlite:maybe-later.db');
    const rows = await db.select<EntryRow[]>(
      'SELECT id, title, content, date, tags, created_at, updated_at FROM entries ORDER BY date, id',
    );
    const entries = rows.map(decodeRow);
    const uniqueIds = new Set(entries.map(entry => entry.id));
    if (uniqueIds.size !== entries.length) {
      throw new Error('В результате чтения обнаружены повторяющиеся ID');
    }
    console.log(`✅ SQLite read: прочитано ${entries.length} записей непосредственно из SQLite; все поля и теги корректны, ID уникальны`);
    console.log('ℹ️ SQLite read: Redux и localStorage не изменены; интерфейс пока работает с localStorage.');
  } catch (error) {
    console.error('❌ SQLite read: ошибка чтения или проверки', error);
  } finally {
    if (db) await db.close();
  }
}
