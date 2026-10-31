import { invoke } from '@tauri-apps/api/core';
import type { Entry } from '../types/entry';

/** SQLite-first create: Redux is updated only after the INSERT succeeds. */
export async function createEntryInSqlite(date: string): Promise<Entry> {
  const now = new Date().toISOString();
  const entry: Entry = {
    id: crypto.randomUUID(), title: 'Новая запись', content: '',
    date, tags: [], createdAt: now, updatedAt: now,
  };

  await invoke('create_secure_entry', { entry });
  return entry;
}
