import Database from '@tauri-apps/plugin-sql';

const STORAGE_KEY = 'maybeLater.calendarColors.v1';
type ColorRow = { date: string; color: string };

/** First-stage migration only. The calendar continues using localStorage until the next release. */
export async function migrateCalendarColorsToSqlite(): Promise<void> {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (raw === null) {
    console.log('ℹ️ SQLite calendar colors migration: localStorage пуст, перенос не требуется');
    return;
  }
  let parsed: unknown;
  try { parsed = JSON.parse(raw); }
  catch { throw new Error('Некорректный JSON цветов календаря в localStorage'); }
  if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error('Некорректный формат цветов календаря в localStorage');
  }
  const colors = Object.entries(parsed as Record<string, unknown>);
  if (!colors.every(([date, color]) => typeof date === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(date) && typeof color === 'string' && color.length > 0)) {
    throw new Error('Некорректные дата или цвет в localStorage календаря');
  }
  const db = await Database.load('sqlite:maybe-later.db');
  try {
    // Do not overwrite previously migrated colors on later startups.
    let inserted = 0;
    for (const [date, color] of colors) {
      const result = await db.execute('INSERT OR IGNORE INTO calendar_colors (date, color) VALUES ($1, $2)', [date, color]);
      inserted += result.rowsAffected;
    }
    const rows = await db.select<ColorRow[]>('SELECT date, color FROM calendar_colors');
    const lookup = new Map(rows.map(row => [row.date, row.color]));
    const missing = colors.filter(([date]) => !lookup.has(date));
    if (missing.length) throw new Error(`Не удалось проверить ${missing.length} цветов календаря в SQLite`);
    console.log(`✅ SQLite calendar colors migration: ${colors.length} дат из localStorage; новых ${inserted}; уже существовали ${colors.length - inserted}; всего в SQLite ${rows.length}`);
  } finally {
    await db.close();
  }
}
