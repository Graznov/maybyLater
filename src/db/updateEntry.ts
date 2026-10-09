import Database from '@tauri-apps/plugin-sql';
import type { Entry } from '../types/entry';

/** Write a complete snapshot so a serialized queue cannot reorder edits. */
export async function updateEntryInSqlite(entry: Entry): Promise<void> {
  const db = await Database.load('sqlite:maybe-later.db');
  try {
    const result = await db.execute(
      `UPDATE entries SET title = $1, content = $2, date = $3,
       tags = $4, updated_at = $5 WHERE id = $6`,
      [entry.title, entry.content, entry.date, JSON.stringify(entry.tags), entry.updatedAt, entry.id],
    );
    if (result.rowsAffected !== 1) throw new Error(`UPDATE affected ${result.rowsAffected} rows for ${entry.id}`);
  } finally {
    await db.close();
  }
}
