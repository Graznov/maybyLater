import { isTauri, invoke } from '@tauri-apps/api/core';
import { loadEntriesFromSqlite } from './loadEntries';
import { loadTagsFromSqlite } from './tagsRepository';
import { waitForSqliteEntryWrites, waitForSqlitePreferencesWrites } from '../app/store';
import { loadPreferencesFromSqlite } from './loadPreferences';
import { store } from '../app/store';
import type { Entry } from '../types/entry';
import type { Tag } from '../types/tag';

export const BACKUP_KEYS = ['maybeLater.entries.v1', 'maybeLater.tags.v1', 'maybeLater.calendarColors.v1', 'maybeLater.preferences.v1'] as const;
type Payload = { entries: Entry[]; tags: Tag[]; preferences: Record<string, unknown> | null; data: Record<string, string | null> };
const object = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);
function unique(values: string[]): boolean { return new Set(values).size === values.length; }
function parseArray(value: unknown, label: string): unknown[] {
  if (!Array.isArray(value)) throw new Error(`Некорректный массив: ${label}`);
  return value;
}
function parseEntries(value: unknown): Entry[] {
  const entries = parseArray(value, 'entries');
  for (const e of entries) {
    if (!object(e) || !['id','title','content','date','createdAt','updatedAt'].every(k => typeof e[k] === 'string') || !e.id ||
      !Array.isArray(e.tags) || !e.tags.every(t => typeof t === 'string')) throw new Error('Некорректная запись в резервной копии');
  }
  if (!unique(entries.map(e => (e as Entry).id))) throw new Error('Повторяющиеся ID записей');
  return entries as Entry[];
}
function parseTags(value: unknown): Tag[] {
  const tags = parseArray(value, 'tags');
  for (const t of tags) {
    if (!object(t) || !['id','name','color'].every(k => typeof t[k] === 'string' && (t[k] as string).trim())) throw new Error('Некорректный тег');
  }
  if (!unique(tags.map(t => (t as Tag).id)) || !unique(tags.map(t => (t as Tag).name))) throw new Error('Повторяющиеся теги');
  return tags as Tag[];
}
export async function makeBackup(): Promise<string> {
  if (isTauri()) { await waitForSqliteEntryWrites(); await waitForSqlitePreferencesWrites(); }
  const entries = isTauri() ? await loadEntriesFromSqlite() : parseEntries(JSON.parse(localStorage.getItem(BACKUP_KEYS[0]) || '[]'));
  const tags = isTauri() ? await loadTagsFromSqlite() : parseTags(JSON.parse(localStorage.getItem(BACKUP_KEYS[1]) || '[]'));
  const data = Object.fromEntries(BACKUP_KEYS.map(key => [key, localStorage.getItem(key)]));
  // Keep the original v1 structure for compatibility with old backups.
  data[BACKUP_KEYS[0]] = JSON.stringify(entries);
  data[BACKUP_KEYS[1]] = JSON.stringify(tags);
  if (isTauri()) data[BACKUP_KEYS[3]] = JSON.stringify(await loadPreferencesFromSqlite(store.getState().preferences));
  return JSON.stringify({ app: 'Maybe Later Journal', version: 2, exportedAt: new Date().toISOString(), data }, null, 2);
}
export function validateBackup(raw: string): Payload {
  const backup: unknown = JSON.parse(raw);
  if (!object(backup) || backup.app !== 'Maybe Later Journal' || !object(backup.data)) throw new Error('Это не резервная копия Maybe Later');
  const data: Record<string, string | null> = {};
  for (const key of BACKUP_KEYS) {
    const value = backup.data[key];
    if (value !== null && typeof value !== 'string') throw new Error(`Некорректное поле ${key}`);
    data[key] = value ?? null;
  }
  if (!data[BACKUP_KEYS[0]] || !data[BACKUP_KEYS[1]]) throw new Error('В файле отсутствуют записи или теги');
  const entries = parseEntries(JSON.parse(data[BACKUP_KEYS[0]]!));
  const tags = parseTags(JSON.parse(data[BACKUP_KEYS[1]]!));
  for (const key of [BACKUP_KEYS[2]]) {
    if (data[key] !== null && !object(JSON.parse(data[key]!))) throw new Error(`Некорректные настройки: ${key}`);
  }
  let preferences: Record<string, unknown> | null = null;
  if (data[BACKUP_KEYS[3]] !== null) {
    const parsed: unknown = JSON.parse(data[BACKUP_KEYS[3]]!);
    if (!object(parsed)) throw new Error('Некорректные настройки в резервной копии');
    const reference = store.getState().preferences as unknown as Record<string, unknown>;
    for (const [key, value] of Object.entries(parsed)) {
      if (!(key in reference) || typeof value !== typeof reference[key]) throw new Error(`Некорректное поле настроек: ${key}`);
    }
    preferences = { ...reference, ...parsed };
  }
  return { entries, tags, preferences, data };
}
export async function restoreBackup(payload: Payload): Promise<string> {
  if (isTauri()) {
    await waitForSqliteEntryWrites();
    await waitForSqlitePreferencesWrites();
    const snapshotPath = await invoke<string>('restore_sqlite_backup', { payload: { entries: payload.entries, tags: payload.tags, preferences: payload.preferences } });
    sessionStorage.setItem('maybeLater.restoreNotice', `Восстановление выполнено. Защитная копия прежней SQLite: ${snapshotPath}`);
    // Preferences are restored inside the same Rust SQLite transaction.
    // A legacy backup without preferences leaves the current SQLite settings intact.
    // Calendar colors remain in localStorage.
    for (const key of [BACKUP_KEYS[2]]) {
      const value = payload.data[key];
      if (value !== null) localStorage.setItem(key, value);
    }
  } else {
    for (const key of BACKUP_KEYS) {
      const value = payload.data[key];
      if (value !== null) localStorage.setItem(key, value);
    }
  }
  window.location.reload();
  return 'ok';
}
