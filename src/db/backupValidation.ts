import type { Entry } from '../types/entry';
import type { Tag } from '../types/tag';

const object = (v: unknown): v is Record<string, unknown> =>
  !!v && typeof v === 'object' && !Array.isArray(v);

const unique = (values: string[]): boolean =>
  new Set(values).size === values.length;

function parseArray(value: unknown, label: string): unknown[] {
  if (!Array.isArray(value)) throw new Error(`Некорректный массив: ${label}`);
  return value;
}

export function parseEntries(value: unknown): Entry[] {
  const entries = parseArray(value, 'entries');
  for (const e of entries) {
    if (!object(e) ||
      !['id', 'title', 'content', 'date', 'createdAt', 'updatedAt'].every(k => typeof e[k] === 'string') ||
      !e.id ||
      !Array.isArray(e.tags) ||
      !e.tags.every(t => typeof t === 'string')) {
      throw new Error('Некорректная запись в резервной копии');
    }
  }
  if (!unique(entries.map(e => (e as Entry).id))) throw new Error('Повторяющиеся ID записей');
  return entries as Entry[];
}

export function parseTags(value: unknown): Tag[] {
  const tags = parseArray(value, 'tags');
  for (const t of tags) {
    if (!object(t) ||
      !['id', 'name', 'color'].every(k => typeof t[k] === 'string' && (t[k] as string).trim())) {
      throw new Error('Некорректный тег');
    }
  }
  if (!unique(tags.map(t => (t as Tag).id)) ||
      !unique(tags.map(t => (t as Tag).name))) {
    throw new Error('Повторяющиеся теги');
  }
  return tags as Tag[];
}

export const BACKUP_KEYS = ['maybeLater.entries.v1', 'maybeLater.tags.v1', 'maybeLater.calendarColors.v1', 'maybeLater.preferences.v1'] as const;
export type Payload = { entries: Entry[]; tags: Tag[]; preferences: Record<string, unknown> | null; calendarColors: Record<string, string> | null; data: Record<string, string | null> };

function parseCalendarColors(value: unknown): Record<string, string> {
  if (!object(value) || !Object.entries(value).every(([date, color]) =>
    /^\d{4}-\d{2}-\d{2}$/.test(date) && typeof color === 'string' && color.length > 0)) {
    throw new Error('Некорректные цвета календаря в резервной копии');
  }
  return value as Record<string, string>;
}

export function validateBackup(raw: string, reference: Record<string, unknown>): Payload {
  const backup: unknown = JSON.parse(raw);
  if (!object(backup) || backup.app !== 'Maybe Later Journal' || !object(backup.data)) throw new Error('Это не резервная копия Maybe Later');
  const data: Record<string, string | null> = {};
  for (const key of BACKUP_KEYS) {
    const value = backup.data[key];
    const optional = key === BACKUP_KEYS[2] || key === BACKUP_KEYS[3];
    if (value === undefined && !optional) throw new Error(`Некорректное поле ${key}`);
    if (value !== undefined && value !== null && typeof value !== 'string') {
      throw new Error(`Некорректное поле ${key}`);
    }
    data[key] = value ?? null;
  }
  if (!data[BACKUP_KEYS[0]] || !data[BACKUP_KEYS[1]]) throw new Error('В файле отсутствуют записи или теги');
  const entries = parseEntries(JSON.parse(data[BACKUP_KEYS[0]]!));
  const tags = parseTags(JSON.parse(data[BACKUP_KEYS[1]]!));
  // Missing colors in a legacy backup means "leave current SQLite colors unchanged".
  const calendarColors = data[BACKUP_KEYS[2]] === null ? null : parseCalendarColors(JSON.parse(data[BACKUP_KEYS[2]]!));
  let preferences: Record<string, unknown> | null = null;
  if (data[BACKUP_KEYS[3]] !== null) {
    const parsed: unknown = JSON.parse(data[BACKUP_KEYS[3]]!);
    if (!object(parsed)) throw new Error('Некорректные настройки в резервной копии');
    for (const [key, value] of Object.entries(parsed)) {
      if (!(key in reference) || typeof value !== typeof reference[key]) throw new Error(`Некорректное поле настроек: ${key}`);
    }
    preferences = { ...reference, ...parsed };
  }
  return { entries, tags, preferences, calendarColors, data };
}
