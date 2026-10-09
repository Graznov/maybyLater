import Database from '@tauri-apps/plugin-sql';
import { PREFERENCES_STORAGE_KEY, type PreferencesState } from '../features/preferences/preferencesSlice';

const SNAPSHOT_KEY = 'preferences.v1';
type PreferenceRow = { key: string; value: string };

/**
 * v10.8.23: one-way snapshot only. Preferences UI still reads and writes localStorage.
 * Never overwrite an existing SQLite snapshot with potentially newer/stale browser state.
 */
export async function testPreferencesMigration(preferences: PreferencesState): Promise<void> {
  const raw = localStorage.getItem(PREFERENCES_STORAGE_KEY);
  // Use the exact preferences already loaded into Redux (including defaults),
  // rather than a second potentially inconsistent localStorage parse.
  const snapshot = JSON.stringify(preferences);
  const db = await Database.load('sqlite:maybe-later.db');
  try {
    const before = await db.select<PreferenceRow[]>(
      'SELECT key, value FROM preferences WHERE key = $1', [SNAPSHOT_KEY],
    );
    if (before.length > 1) throw new Error('Нарушена уникальность ключа настроек SQLite');
    let inserted = false;
    if (before.length === 0) {
      const result = await db.execute(
        'INSERT OR IGNORE INTO preferences (key, value) VALUES ($1, $2)',
        [SNAPSHOT_KEY, snapshot],
      );
      inserted = result.rowsAffected === 1;
    }
    const after = await db.select<PreferenceRow[]>(
      'SELECT key, value FROM preferences WHERE key = $1', [SNAPSHOT_KEY],
    );
    if (after.length !== 1) throw new Error('Снимок настроек не найден в SQLite');
    const parsed: unknown = JSON.parse(after[0].value);
    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new Error('Некорректный JSON настроек в SQLite');
    }
    if (inserted && after[0].value !== snapshot) {
      throw new Error('Проверка только что записанных настроек не пройдена');
    }
    console.log(`✅ SQLite preferences migration: ${inserted ? 'создан и проверен новый снимок' : 'существующий снимок прочитан, без перезаписи'}; ${Object.keys(parsed).length} полей`);
    if (raw === null) console.info('ℹ️ SQLite preferences: localStorage не содержит настроек; сохранены текущие значения Redux с умолчаниями.');
    if (!inserted && after[0].value !== snapshot) console.info('ℹ️ SQLite preferences: снимок отличается от текущих localStorage-настроек; это ожидаемо до переключения интерфейса на SQLite.');
    console.info('ℹ️ SQLite preferences: интерфейс продолжает использовать localStorage; записи и теги не изменялись.');
  } finally {
    await db.close();
  }
}
