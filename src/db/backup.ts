import { isTauri, invoke } from '@tauri-apps/api/core';
import { loadEntriesFromSqlite } from './loadEntries';
import { loadTagsFromSqlite, waitForSqliteTagWrites } from './tagsRepository';
import { waitForSqliteEntryWrites, waitForSqlitePreferencesWrites, waitForSqliteCalendarWrites } from '../app/store';
import { loadPreferencesFromSqlite } from './loadPreferences';
import { store } from '../app/store';
import { BACKUP_KEYS, parseEntries, parseTags, validateBackup as validateBackupData } from './backupValidation';
import type { Payload } from './backupValidation';
export { BACKUP_KEYS } from './backupValidation';
import type { Entry } from '../types/entry';
import type { Tag } from '../types/tag';

async function readCalendarColorsFromSqlite(): Promise<Record<string, string>> {
  const rows = await invoke<[string, string][]>('load_secure_calendar_colors');
  return Object.fromEntries(rows);
}

export async function makeBackup(): Promise<string> {
  if (isTauri()) { await waitForSqliteEntryWrites(); await waitForSqliteTagWrites(); await waitForSqlitePreferencesWrites(); await waitForSqliteCalendarWrites(); }
  const entries = isTauri() ? await loadEntriesFromSqlite() : parseEntries(JSON.parse(localStorage.getItem(BACKUP_KEYS[0]) || '[]'));
  const tags = isTauri() ? await loadTagsFromSqlite() : parseTags(JSON.parse(localStorage.getItem(BACKUP_KEYS[1]) || '[]'));
  const data = Object.fromEntries(BACKUP_KEYS.map(key => [key, localStorage.getItem(key)]));
  // Keep the original v1 structure for compatibility with old backups.
  data[BACKUP_KEYS[0]] = JSON.stringify(entries);
  data[BACKUP_KEYS[1]] = JSON.stringify(tags);
  if (isTauri()) data[BACKUP_KEYS[2]] = JSON.stringify(await readCalendarColorsFromSqlite());
  if (isTauri()) data[BACKUP_KEYS[3]] = JSON.stringify(await loadPreferencesFromSqlite(store.getState().preferences));
  return JSON.stringify({ app: 'Maybe Later Journal', version: 2, exportedAt: new Date().toISOString(), data }, null, 2);
}
export function validateBackup(raw: string): Payload {
  return validateBackupData(raw, store.getState().preferences as unknown as Record<string, unknown>);
}
export async function restoreBackup(payload: Payload, password: string): Promise<string> {
  if (isTauri()) {
    await waitForSqliteEntryWrites();
    await waitForSqliteTagWrites();
    await waitForSqlitePreferencesWrites();
    await waitForSqliteCalendarWrites();
    if (!password) throw new Error('Введите пароль дневника для восстановления резервной копии');
    const snapshotPath = await invoke<string>('restore_sqlite_backup', {
      payload: {
        entries: payload.entries,
        tags: payload.tags,
        preferences: payload.preferences,
        calendarColors: payload.calendarColors
      },
      password
    });
    const english = store.getState().preferences.language === 'en';
    sessionStorage.setItem(
      'maybeLater.restoreNotice',
      english
        ? `Restore completed. Previous SQLite safety snapshot: ${snapshotPath}`
        : `Восстановление выполнено. Защитная копия прежней SQLite: ${snapshotPath}`
    );
    // Preferences are restored inside the same Rust SQLite transaction.
    // A legacy backup without preferences leaves the current SQLite settings intact.
    // Calendar colors are restored atomically in SQLite. Do not write stale
    // localStorage colors: the SQLite-writer handover marker is authoritative.
  } else {
    for (const key of BACKUP_KEYS) {
      const value = payload.data[key];
      if (value !== null) localStorage.setItem(key, value);
    }
  }
  window.location.reload();
  return 'ok';
}
