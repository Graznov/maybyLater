import { invoke } from '@tauri-apps/api/core';
import { PREFERENCES_STORAGE_KEY, type PreferencesState } from '../features/preferences/preferencesSlice';


const EXPECTED_KEYS = [
  'journalName', 'theme', 'language', 'firstDayOfWeek', 'timeFormat',
  'editorFontSize', 'editorWidth', 'sidebarBackground', 'windowTransparency',
  'autosave', 'autosaveDelay', 'showToolbar', 'spellcheck', 'showWordCount',
  'showEntryTime',
] as const satisfies readonly (keyof PreferencesState)[];

/** Reject malformed snapshots rather than silently replacing SQLite preferences. */
export function parsePreferencesSnapshot(raw: string): PreferencesState {
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    throw new Error('Некорректный JSON настроек SQLite');
  }
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('Настройки SQLite должны быть объектом');
  }
  const parsed = value as Record<string, unknown>;
  if (Object.keys(parsed).length !== EXPECTED_KEYS.length ||
      EXPECTED_KEYS.some((key) => !Object.prototype.hasOwnProperty.call(parsed, key))) {
    throw new Error('Неверный набор полей настроек SQLite');
  }

  const oneOf = (key: keyof PreferencesState, options: readonly string[]) => {
    if (!options.includes(parsed[key] as string)) {
      throw new Error(`Недопустимое значение настройки SQLite: ${key}`);
    }
  };
  if (typeof parsed.journalName !== 'string') {
    throw new Error('Недопустимое значение настройки SQLite: journalName');
  }
  // Forward compatibility: an older build may encounter a theme saved by a newer build.
  // Fall back in memory only; never rewrite the SQLite snapshot during loading.
  if (!['system', 'light', 'dark', 'cream'].includes(parsed.theme as string)) {
    console.warn('Неизвестная тема в SQLite; временно используется системная тема');
    parsed.theme = 'system';
  }
  oneOf('language', ['ru', 'en']);
  oneOf('firstDayOfWeek', ['monday', 'sunday']);
  oneOf('timeFormat', ['24h', '12h']);
  oneOf('editorFontSize', ['small', 'medium', 'large']);
  oneOf('editorWidth', ['compact', 'normal', 'wide']);
  for (const key of [
    'sidebarBackground', 'windowTransparency', 'autosave', 'showToolbar',
    'spellcheck', 'showWordCount', 'showEntryTime',
  ] as const) {
    if (typeof parsed[key] !== 'boolean') {
      throw new Error(`Недопустимое значение настройки SQLite: ${key}`);
    }
  }
  if (typeof parsed.autosaveDelay !== 'number' ||
      !Number.isFinite(parsed.autosaveDelay) || parsed.autosaveDelay < 0) {
    throw new Error('Недопустимое значение настройки SQLite: autosaveDelay');
  }
  return parsed as PreferencesState;
}

/** v10.8.25: SQLite is authoritative; localStorage is only a one-time fallback. */
export async function loadPreferencesFromSqlite(current: PreferencesState): Promise<PreferencesState> {
  const existing = await invoke<string | null>("load_secure_preferences");
  if (existing === null) {
    const raw = localStorage.getItem(PREFERENCES_STORAGE_KEY);
    const initial = raw === null ? current : parsePreferencesSnapshot(raw);
    await invoke("insert_initial_secure_preferences", { value: JSON.stringify(initial) });
    console.log("✅ SQLite preferences: выполнена первичная миграция из localStorage");
  }
  const value = await invoke<string | null>("load_secure_preferences");
  if (value === null) throw new Error("Не удалось прочитать настройки SQLite");
  const loaded = parsePreferencesSnapshot(value);
  console.log(`✅ SQLite preferences read: ${Object.keys(loaded).length} полей загружено из SQLite в Redux`);
  return loaded;
}
