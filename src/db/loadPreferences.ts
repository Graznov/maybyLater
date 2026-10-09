import Database from '@tauri-apps/plugin-sql';
import { PREFERENCES_STORAGE_KEY, type PreferencesState } from '../features/preferences/preferencesSlice';

type Row = { value: string };
const KEY = 'preferences.v1';

function parseSnapshot(raw: string): PreferencesState {
  const value: unknown = JSON.parse(raw);
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('Некорректный JSON настроек');
  }
  const parsed = value as Record<string, unknown>;
  if (Object.keys(parsed).length !== 15) throw new Error('Неверное число полей настроек');
  return parsed as PreferencesState;
}

/** v10.8.25: SQLite is authoritative; localStorage is only a one-time fallback. */
export async function loadPreferencesFromSqlite(current: PreferencesState): Promise<PreferencesState> {
  const db = await Database.load('sqlite:maybe-later.db');
  try {
    // SQLite is now authoritative. Never overwrite it with potentially stale localStorage.
    const existing = await db.select<Row[]>('SELECT value FROM preferences WHERE key = $1', [KEY]);
    if (!existing.length) {
      const raw = localStorage.getItem(PREFERENCES_STORAGE_KEY);
      const initial = raw === null ? current : JSON.parse(raw) as PreferencesState;
      await db.execute('INSERT OR IGNORE INTO preferences (key, value) VALUES ($1, $2)', [KEY, JSON.stringify(initial)]);
      console.log('✅ SQLite preferences: выполнена первичная миграция из localStorage');
    }
    const rows = await db.select<Row[]>('SELECT value FROM preferences WHERE key = $1', [KEY]);
    if (rows.length !== 1) throw new Error('Не удалось прочитать настройки SQLite');
    const loaded = parseSnapshot(rows[0].value);
    console.log(`✅ SQLite preferences read: ${Object.keys(loaded).length} полей загружено из SQLite в Redux`);
    return loaded;
  } finally {
    await db.close();
  }
}
