import Database from '@tauri-apps/plugin-sql';
import type { Tag } from '../types/tag';
import { seedTags } from '../data/tags';

const STORAGE_KEY = 'maybeLater.tags.v1';
type TagRow = { id: string; name: string; color: string };

/** One-way, non-destructive verification. The UI still uses localStorage for tags. */
export async function testTagsMigration(): Promise<void> {
  const raw = localStorage.getItem(STORAGE_KEY);
  const source: unknown = raw === null ? seedTags : JSON.parse(raw);
  if (!Array.isArray(source) || !source.every((item): item is Tag =>
    item !== null && typeof item === 'object' &&
    typeof item.id === 'string' && item.id.length > 0 &&
    typeof item.name === 'string' && item.name.length > 0 &&
    typeof item.color === 'string' && item.color.length > 0
  )) {
    throw new Error('Некорректный справочник тегов в localStorage');
  }
  const ids = new Set(source.map(tag => tag.id));
  if (ids.size !== source.length) throw new Error('Повторяющиеся ID тегов в localStorage');

  const db = await Database.load('sqlite:maybe-later.db');
  try {
    const before = await db.select<TagRow[]>('SELECT id, name, color FROM tags');
    const existing = new Map(before.map(tag => [tag.id, tag]));
    for (const tag of source) {
      const old = existing.get(tag.id);
      if (old && (old.name !== tag.name || old.color !== tag.color)) {
        throw new Error(`Конфликт данных тега ${tag.id}: SQLite отличается от localStorage. Данные не перезаписаны.`);
      }
    }

    let inserted = 0;
    for (const tag of source) {
      if (existing.has(tag.id)) continue;
      const result = await db.execute(
        'INSERT OR IGNORE INTO tags (id, name, color) VALUES ($1, $2, $3)',
        [tag.id, tag.name, tag.color],
      );
      inserted += result.rowsAffected;
    }

    const after = await db.select<TagRow[]>('SELECT id, name, color FROM tags ORDER BY id');
    const byId = new Map(after.map(tag => [tag.id, tag]));
    for (const tag of source) {
      const saved = byId.get(tag.id);
      if (!saved || saved.name !== tag.name || saved.color !== tag.color) {
        throw new Error(`Ошибка проверки сохранённого тега: ${tag.id}`);
      }
    }
    console.log(`✅ SQLite tags migration: проверено ${source.length}, новых ${inserted}, уже существовали ${source.length - inserted}; названия и цвета совпадают`);
    console.log(`✅ SQLite tags read: непосредственно из SQLite прочитано ${after.length} тегов; уникальных ID ${byId.size}`);
    console.info('ℹ️ SQLite tags: интерфейс и операции с тегами пока используют localStorage; исходные данные не удалены.');
  } finally {
    await db.close();
  }
}
