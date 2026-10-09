import Database from '@tauri-apps/plugin-sql';
import type { CalendarColors } from '../features/calendar/calendarSlice';

const STORAGE_KEY = 'maybeLater.calendarColors.v1';
const HANDOVER_KEY = 'maybeLater.calendarColors.sqliteWriter.v1';
const SQLITE_HANDOVER_KEY = 'calendar_colors.sqliteWriter.v1';
type ColorRow = { date: string; color: string };

/** Import legacy browser colors only into an SQLite database that has never owned colors. */
export async function migrateCalendarColorsToSqlite(): Promise<CalendarColors> {
  const db = await Database.load('sqlite:maybe-later.db');
  try {
    const metadata = await db.select<{ value: string }[]>(
      'SELECT value FROM app_metadata WHERE key = $1', [SQLITE_HANDOVER_KEY],
    );
    const existing = await db.select<ColorRow[]>('SELECT date, color FROM calendar_colors ORDER BY date');
    const browserHandedOver = localStorage.getItem(HANDOVER_KEY) === '1';

    // The database marker survives localStorage clearing. Existing SQLite colors
    // also take precedence if an older release had not yet stored that marker.
    if (metadata.length === 0 && existing.length === 0 && !browserHandedOver) {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw !== null) {
        let parsed: unknown;
        try { parsed = JSON.parse(raw); }
        catch { throw new Error('Некорректный JSON цветов календаря в localStorage'); }
        if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
          throw new Error('Некорректный формат цветов календаря в localStorage');
        }
        const colors = Object.entries(parsed as Record<string, unknown>);
        if (!colors.every(([date, color]) => /^\d{4}-\d{2}-\d{2}$/.test(date) && typeof color === 'string' && color.length > 0)) {
          throw new Error('Некорректные дата или цвет в localStorage календаря');
        }
        for (const [date, color] of colors) {
          await db.execute(
            'INSERT INTO calendar_colors (date, color) VALUES ($1, $2) ON CONFLICT(date) DO UPDATE SET color = excluded.color',
            [date, color],
          );
        }
      }
    }

    // Mark SQLite as authoritative even if its calendar is intentionally empty.
    await db.execute(
      'INSERT INTO app_metadata (key, value) VALUES ($1, $2) ON CONFLICT(key) DO UPDATE SET value = excluded.value',
      [SQLITE_HANDOVER_KEY, '1'],
    );
    const rows = await db.select<ColorRow[]>('SELECT date, color FROM calendar_colors ORDER BY date');
    localStorage.setItem(HANDOVER_KEY, '1');
    console.log(`✅ SQLite calendar colors read: ${rows.length} дат загружено из SQLite в Redux`);
    return Object.fromEntries(rows.map(row => [row.date, row.color]));
  } finally {
    await db.close();
  }
}
