import Database from '@tauri-apps/plugin-sql';
import type { Entry } from '../types/entry';

/** Duplicate a fully saved entry, preserving its content and tags. */
export async function duplicateEntryInSqlite(source: Entry): Promise<Entry> {
  const now = new Date().toISOString();
  const copy: Entry = {
    ...source,
    id: crypto.randomUUID(),
    title: `${source.title || 'Без названия'} — копия`,
    tags: [...source.tags],
    createdAt: now,
    updatedAt: now,
  };
  const db = await Database.load('sqlite:maybe-later.db');
  try {
    const result = await db.execute(
      `INSERT INTO entries (id, title, content, date, tags, created_at, updated_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7)`,
      [copy.id, copy.title, copy.content, copy.date,
       JSON.stringify(copy.tags), copy.createdAt, copy.updatedAt],
    );
    if (result.rowsAffected !== 1) throw new Error(`Duplicate INSERT affected ${result.rowsAffected} rows`);
    return copy;
  } finally {
    await db.close();
  }
}
