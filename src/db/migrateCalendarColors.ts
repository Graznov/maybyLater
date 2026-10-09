import Database from '@tauri-apps/plugin-sql';
import type { CalendarColors } from '../features/calendar/calendarSlice';

const STORAGE_KEY = 'maybeLater.calendarColors.v1';
type ColorRow = { date: string; color: string };

/** One-time handover from localStorage to SQLite; never replay stale data after handover. */
export async function migrateCalendarColorsToSqlite(): Promise<CalendarColors> {
  const handoverKey = 'maybeLater.calendarColors.sqliteWriter.v1';
  const handedOver = localStorage.getItem(handoverKey) === '1';
  const raw = handedOver ? null : localStorage.getItem(STORAGE_KEY);
  let colors: [string, string][] | null = null;
  if (raw !== null) {
    let parsed: unknown;
    try { parsed = JSON.parse(raw); }
    catch { throw new Error('Некорректный JSON цветов календаря в localStorage'); }
    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new Error('Некорректный формат цветов календаря в localStorage');
    }
    const entries = Object.entries(parsed as Record<string, unknown>);
    if (!entries.every(([date, color]) => /^\d{4}-\d{2}-\d{2}$/.test(date) && typeof color === 'string' && color.length > 0)) {
      throw new Error('Некорректные дата или цвет в localStorage календаря');
    }
    colors = entries as [string, string][];
  }
  const db = await Database.load('sqlite:maybe-later.db');
  try {
    // v10.8.29: localStorage remains the writer, so reconcile changes and
    // removals before SQLite becomes the source for the initial Redux state.
    // Missing localStorage is NOT an instruction to erase existing SQLite data.
    if (colors !== null) {
      for (const [date, color] of colors) {
        await db.execute(
          'INSERT INTO calendar_colors (date, color) VALUES ($1, $2) ON CONFLICT(date) DO UPDATE SET color = excluded.color',
          [date, color],
        );
      }
      // A date removed from localStorage must also be removed from SQLite.
      // All inputs are validated above; placeholders avoid SQL interpolation.
      if (colors.length === 0) {
        await db.execute('DELETE FROM calendar_colors');
      } else {
        const placeholders = colors.map((_, i) => `$${i + 1}`).join(', ');
        await db.execute(`DELETE FROM calendar_colors WHERE date NOT IN (${placeholders})`, colors.map(([date]) => date));
      }
    }
    const rows = await db.select<ColorRow[]>('SELECT date, color FROM calendar_colors ORDER BY date');
    const result: CalendarColors = Object.fromEntries(rows.map(row => [row.date, row.color]));
    if (colors !== null && (rows.length !== colors.length || colors.some(([date, color]) => result[date] !== color))) {
      throw new Error('Цвета календаря SQLite не совпадают с localStorage');
    }
    // Set the marker only after SQLite has been read and verified successfully.
    // On future launches SQLite is authoritative, including when all colors were removed.
    localStorage.setItem(handoverKey, '1');
    console.log(`✅ SQLite calendar colors read: ${rows.length} дат загружено из SQLite в Redux; запись цветов теперь в SQLite`);
    return result;
  } finally {
    await db.close();
  }
}
