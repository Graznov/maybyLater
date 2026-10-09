import Database from '@tauri-apps/plugin-sql';
import type { PreferencesState } from '../features/preferences/preferencesSlice';

/** One complete snapshot per change; callers serialize writes to prevent stale overwrites. */
export async function savePreferencesToSqlite(preferences: PreferencesState): Promise<void> {
  const db = await Database.load('sqlite:maybe-later.db');
  try {
    await db.execute(
      'INSERT INTO preferences (key, value) VALUES ($1, $2) ON CONFLICT(key) DO UPDATE SET value = excluded.value',
      ['preferences.v1', JSON.stringify(preferences)],
    );
  } finally {
    await db.close();
  }
}
