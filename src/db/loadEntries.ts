import Database from '@tauri-apps/plugin-sql';
import type { Entry } from '../types/entry';

type EntryRow = {
  id: string; title: string; content: string; date: string; tags: string;
  created_at: string; updated_at: string;
};

export async function loadEntriesFromSqlite(): Promise<Entry[]> {
  const db = await Database.load('sqlite:maybe-later.db');
  try {
    const rows = await db.select<EntryRow[]>(
      'SELECT id, title, content, date, tags, created_at, updated_at FROM entries ORDER BY date DESC, created_at DESC, id',
    );
    const ids = new Set<string>();
    return rows.map(row => {
      const tags: unknown = JSON.parse(row.tags);
      if (!Array.isArray(tags) || !tags.every(tag => typeof tag === 'string')) {
        throw new Error(`Неверный формат тегов записи ${row.id}`);
      }
      if (![row.id, row.title, row.content, row.date, row.created_at, row.updated_at].every(value => typeof value === 'string') || !row.id || ids.has(row.id)) {
        throw new Error(`Некорректные поля или повторяющийся ID: ${String(row.id)}`);
      }
      ids.add(row.id);
      return {
        id: row.id, title: row.title, content: row.content, date: row.date,
        tags, createdAt: row.created_at, updatedAt: row.updated_at,
      };
    });
  } finally {
    await db.close();
  }
}
