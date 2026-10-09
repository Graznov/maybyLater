import Database from '@tauri-apps/plugin-sql';

/** Delete exactly one existing entry. The UI changes only after success. */
export async function deleteEntryInSqlite(id: string): Promise<void> {
  const db = await Database.load('sqlite:maybe-later.db');
  try {
    const result = await db.execute('DELETE FROM entries WHERE id = $1', [id]);
    if (result.rowsAffected !== 1) throw new Error(`DELETE affected ${result.rowsAffected} rows for ${id}`);
  } finally {
    await db.close();
  }
}
