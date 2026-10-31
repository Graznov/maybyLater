import { invoke } from '@tauri-apps/api/core';

/** One SQL statement per action; middleware serializes actions in dispatch order. */
export async function saveCalendarColor(date: string, color: string | null): Promise<void> {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) throw new Error('Некорректная дата календаря');
  if (color !== null && !color) throw new Error('Некорректный цвет календаря');
  await invoke("save_secure_calendar_color", { date, color });
}
