import Database from '@tauri-apps/plugin-sql';
import type { Entry } from '../types/entry';

/** Atomically change the tag JSON of all affected rows in one SQL statement. */
export async function changeTagInEntries(
  entries: Entry[], oldName: string, newName: string | null,
): Promise<number> {
  const affected = entries.filter(entry => entry.tags.includes(oldName));
  if (affected.length === 0) return 0;
  const db = await Database.load('sqlite:maybe-later.db');
  try {
    // Single UPDATE is atomic even if one row fails. Bind every value.
    const now = new Date().toISOString();
    const replacements = affected.map(entry => ({
      id: entry.id,
      tags: JSON.stringify(newName === null
        ? entry.tags.filter(tag => tag !== oldName)
        : entry.tags.map(tag => tag === oldName ? newName : tag)),
    }));
    const caseSql = replacements.map((_, i) => `WHEN $${i * 2 + 1} THEN $${i * 2 + 2}`).join(' ');
    const idsSql = replacements.map((_, i) => `$${replacements.length * 2 + i + 2}`).join(', ');
    const params: string[] = replacements.flatMap(row => [row.id, row.tags]);
    params.push(now, ...replacements.map(row => row.id));
    const result = await db.execute(
      `UPDATE entries SET tags = CASE id ${caseSql} END, updated_at = $${replacements.length * 2 + 1} WHERE id IN (${idsSql})`,
      params,
    );
    if (result.rowsAffected !== replacements.length) {
      throw new Error(`Изменено ${result.rowsAffected} из ${replacements.length} записей. Перезагрузите приложение перед продолжением.`);
    }
    return affected.length;
  } finally {
    await db.close();
  }
}
