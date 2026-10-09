import Database from '@tauri-apps/plugin-sql';
import type { Entry } from '../types/entry';

/** SQLite-first create: Redux is updated only after the INSERT succeeds. */
export async function createEntryInSqlite(date: string): Promise<Entry> {
  const now = new Date().toISOString();
  const entry: Entry = {
    id: crypto.randomUUID(), title: 'Новая запись', content: '',
    date, tags: [], createdAt: now, updatedAt: now,
  };
  const db = await Database.load('sqlite:maybe-later.db');
  try {
    const result = await db.execute(
      `INSERT INTO entries (id, title, content, date, tags, created_at, updated_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7)`,
      [entry.id, entry.title, entry.content, entry.date,
       JSON.stringify(entry.tags), entry.createdAt, entry.updatedAt],
    );
    if (result.rowsAffected !== 1) throw new Error(`INSERT affected ${result.rowsAffected} rows`);
    return entry;
  } finally {
    await db.close();
  }
}
