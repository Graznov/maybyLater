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

/** v10.8.24: localStorage is still the write authority; synchronize it before reading SQLite. */
export async function loadPreferencesFromSqlite(current: PreferencesState): Promise<PreferencesState> {
  const db = await Database.load('sqlite:maybe-later.db');
  try {
    const localRaw = localStorage.getItem(PREFERENCES_STORAGE_KEY);
    if (localRaw !== null) {
      // Redux has already applied defaults to the localStorage state.
      const fresh = JSON.stringify(current);
      await db.execute(
        'INSERT INTO preferences (key, value) VALUES ($1, $2) ON CONFLICT(key) DO UPDATE SET value = excluded.value',
        [KEY, fresh],
      );
      console.log('✅ SQLite preferences sync: актуальные настройки localStorage записаны в SQLite');
    } else {
      const existing = await db.select<Row[]>('SELECT value FROM preferences WHERE key = $1', [KEY]);
      if (!existing.length) {
        await db.execute('INSERT INTO preferences (key, value) VALUES ($1, $2)', [KEY, JSON.stringify(current)]);
        console.log('✅ SQLite preferences sync: сохранены настройки по умолчанию');
      } else {
        console.log('ℹ️ SQLite preferences sync: localStorage отсутствует; используется сохранённый снимок SQLite');
      }
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
