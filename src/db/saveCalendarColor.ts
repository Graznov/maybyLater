import Database from '@tauri-apps/plugin-sql';

/** One SQL statement per action; middleware serializes actions in dispatch order. */
export async function saveCalendarColor(date: string, color: string | null): Promise<void> {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) throw new Error('Некорректная дата календаря');
  if (color !== null && !color) throw new Error('Некорректный цвет календаря');
  const db = await Database.load('sqlite:maybe-later.db');
  try {
    if (color === null) {
      await db.execute('DELETE FROM calendar_colors WHERE date = $1', [date]);
    } else {
      await db.execute('INSERT INTO calendar_colors (date, color) VALUES ($1, $2) ON CONFLICT(date) DO UPDATE SET color = excluded.color', [date, color]);
    }
  } finally {
    await db.close();
  }
}
