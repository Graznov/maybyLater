import Database from '@tauri-apps/plugin-sql';
import type { Tag } from '../types/tag';

const URL = 'sqlite:maybe-later.db';

// Track tag mutations so backup/restore never reads a half-finished tag operation.
const pendingTagWrites = new Set<Promise<void>>();
function trackTagWrite(write: Promise<void>): Promise<void> {
  pendingTagWrites.add(write);
  // Register cleanup on both outcomes without creating an unhandled rejection.
  void write.then(() => pendingTagWrites.delete(write), () => pendingTagWrites.delete(write));
  return write;
}

export async function waitForSqliteTagWrites(): Promise<void> {
  while (pendingTagWrites.size > 0) {
    const results = await Promise.allSettled([...pendingTagWrites]);
    const failure = results.find((result): result is PromiseRejectedResult => result.status === 'rejected');
    if (failure) throw failure.reason;
  }
}


export async function loadTagsFromSqlite(): Promise<Tag[]> {
  const db = await Database.load(URL);
  try {
    const rows = await db.select<Tag[]>('SELECT id, name, color FROM tags ORDER BY rowid');
    const ids = new Set<string>();
    for (const row of rows) {
      if (!row.id || !row.name || !row.color || ids.has(row.id)) throw new Error('Некорректный справочник тегов SQLite');
      ids.add(row.id);
    }
    return rows;
  } finally { await db.close(); }
}

export function createTagInSqlite(tag: Tag): Promise<void> {
  return trackTagWrite(createTagImpl(tag));
}
async function createTagImpl(tag: Tag): Promise<void> {
  const db = await Database.load(URL);
  try { await db.execute('INSERT INTO tags (id, name, color) VALUES ($1, $2, $3)', [tag.id, tag.name, tag.color]); }
  finally { await db.close(); }
}

/** SQLite triggers update entries.tags in the same transaction as this UPDATE. */
export function updateTagInSqlite(tag: Tag): Promise<void> {
  return trackTagWrite(updateTagImpl(tag));
}
async function updateTagImpl(tag: Tag): Promise<void> {
  const db = await Database.load(URL);
  try {
    const result = await db.execute('UPDATE tags SET name = $1, color = $2 WHERE id = $3', [tag.name, tag.color, tag.id]);
    if (result.rowsAffected !== 1) throw new Error('Тег не найден в SQLite');
  } finally { await db.close(); }
}

/** SQLite trigger removes the tag from entries atomically with DELETE. */
export function deleteTagInSqlite(id: string): Promise<void> {
  return trackTagWrite(deleteTagImpl(id));
}
async function deleteTagImpl(id: string): Promise<void> {
  const db = await Database.load(URL);
  try {
    const result = await db.execute('DELETE FROM tags WHERE id = $1', [id]);
    if (result.rowsAffected !== 1) throw new Error('Тег не найден в SQLite');
  } finally { await db.close(); }
}
