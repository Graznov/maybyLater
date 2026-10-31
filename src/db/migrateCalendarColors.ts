import { invoke } from '@tauri-apps/api/core';
import type { CalendarColors } from '../features/calendar/calendarSlice';

const STORAGE_KEY = 'maybeLater.calendarColors.v1';
const HANDOVER_KEY = 'maybeLater.calendarColors.sqliteWriter.v1';

type ColorRow = [date: string, color: string];

/** Import legacy browser colors only when the secure SQLite database has never owned colors. */
export async function migrateCalendarColorsToSqlite(): Promise<CalendarColors> {
  const browserHandedOver = localStorage.getItem(HANDOVER_KEY) === '1';
  const needsImport = !browserHandedOver && await invoke<boolean>("needs_secure_calendar_color_import");
  const raw = needsImport ? localStorage.getItem(STORAGE_KEY) : null;
  let legacyColors: ColorRow[] = [];

  if (raw !== null) {
    let parsed: unknown;
    try { parsed = JSON.parse(raw); }
    catch { throw new Error('Некорректный JSON цветов календаря в localStorage'); }

    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new Error('Некорректный формат цветов календаря в localStorage');
    }

    const colors = Object.entries(parsed as Record<string, unknown>);

    if (!colors.every(([date, color]) =>
      /^\d{4}-\d{2}-\d{2}$/.test(date) &&
      typeof color === 'string' &&
      color.length > 0
    )) {
      throw new Error('Некорректные дата или цвет в localStorage календаря');
    }

    legacyColors = colors as ColorRow[];
  }

  const rows = await invoke<ColorRow[]>('migrate_secure_calendar_colors', {
    legacyColors,
    browserHandedOver,
  });

  localStorage.setItem(HANDOVER_KEY, '1');
  console.log(`✅ SQLite calendar colors read: ${rows.length} дат загружено из SQLite в Redux`);

  return Object.fromEntries(rows);
}
