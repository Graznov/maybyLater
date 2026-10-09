import Database from '@tauri-apps/plugin-sql';
import type { Entry } from '../types/entry';

const STORAGE_KEY = 'maybeLater.entries.v1';
type Row = { id: string; title: string; content: string; date: string; tags: string; created_at: string; updated_at: string };

function validate(value: unknown): value is Entry {
  if (!value || typeof value !== 'object') return false;
  const e = value as Record<string, unknown>;
  return ['id', 'title', 'content', 'date', 'createdAt', 'updatedAt'].every(k => typeof e[k] === 'string')
    && (e.id as string).length > 0
    && Array.isArray(e.tags) && e.tags.every((tag: unknown) => typeof tag === 'string');
}

function matches(entry: Entry, row: Row): boolean {
  try {
    return entry.id === row.id && entry.title === row.title && entry.content === row.content
      && entry.date === row.date && entry.createdAt === row.created_at && entry.updatedAt === row.updated_at
      && JSON.stringify(entry.tags) === JSON.stringify(JSON.parse(row.tags));
  } catch { return false; }
}

/** Diagnostic-only copy. Redux and localStorage remain the application's source of truth. */
export async function testDataMigration(): Promise<void> {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (raw === null) {
    console.log('ℹ️ SQLite migration: localStorage не содержит ключ записей, перенос пропущен');
    return;
  }

  let parsed: unknown;
  try { parsed = JSON.parse(raw); } catch {
    console.error('❌ SQLite migration: некорректный JSON в localStorage, перенос отменён');
    return;
  }
  if (!Array.isArray(parsed) || !parsed.every(validate)) {
    console.error('❌ SQLite migration: неверная структура записей, перенос отменён');
    return;
  }
  const entries: Entry[] = parsed;
  const ids = entries.map(e => e.id);
  if (new Set(ids).size !== ids.length) {
    console.error('❌ SQLite migration: повторяющиеся ID в localStorage, перенос отменён');
    return;
  }
  if (entries.length === 0) {
    console.log('ℹ️ SQLite migration: в localStorage 0 записей, перенос не требуется');
    return;
  }

  const db = await Database.load('sqlite:maybe-later.db');
  try {
    // Read the target table before writing. Never silently overwrite conflicting data.
    const before = await db.select<Row[]>('SELECT * FROM entries');
    const existing = new Map(before.map(row => [row.id, row]));
    const conflicts = entries.filter(e => existing.has(e.id) && !matches(e, existing.get(e.id)!));
    if (conflicts.length) {
      console.error(`❌ SQLite migration: ${conflicts.length} конфликтующих ID; копирование отменено`, conflicts.map(e => e.id));
      return;
    }

    // One SQL statement = one atomic SQLite statement, regardless of entry count.
    // JSON1's json_each is available in SQLite bundled with modern Tauri SQL builds.
    const payload = entries.filter(e => !existing.has(e.id)).map(e => ({
      id: e.id, title: e.title, content: e.content, date: e.date,
      tags: JSON.stringify(e.tags), created_at: e.createdAt, updated_at: e.updatedAt,
    }));
    if (payload.length) {
      await db.execute(`
        INSERT INTO entries (id, title, content, date, tags, created_at, updated_at)
        SELECT json_extract(value, '$.id'), json_extract(value, '$.title'),
               json_extract(value, '$.content'), json_extract(value, '$.date'),
               json_extract(value, '$.tags'), json_extract(value, '$.created_at'),
               json_extract(value, '$.updated_at')
        FROM json_each($1)
        WHERE 1 = 1
        ON CONFLICT(id) DO NOTHING
      `, [JSON.stringify(payload)]);
    }

    const after = await db.select<Row[]>('SELECT * FROM entries');
    const saved = new Map(after.map(row => [row.id, row]));
    const mismatches = entries.filter(e => !saved.has(e.id) || !matches(e, saved.get(e.id)!));
    if (mismatches.length) {
      console.error('❌ SQLite migration: проверка данных не пройдена', mismatches.map(e => e.id));
      return;
    }
    console.log(`✅ SQLite migration: проверено ${entries.length}, новых ${payload.length}, уже существовали ${entries.length - payload.length}; все поля совпадают`);
    console.log('ℹ️ localStorage не изменён. Приложение пока продолжает работать с localStorage.');
  } catch (error) {
    console.error('❌ SQLite migration: ошибка копирования или проверки', error);
  } finally {
    await db.close();
  }
}
