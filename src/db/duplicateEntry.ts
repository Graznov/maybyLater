import { invoke } from '@tauri-apps/api/core';
import type { Entry } from '../types/entry';

/** Duplicate a fully saved entry, preserving its content and tags. */
export async function duplicateEntryInSqlite(source: Entry): Promise<Entry> {
  const now = new Date().toISOString();
  const copy: Entry = {
    ...source,
    id: crypto.randomUUID(),
    title: `${source.title || 'Без названия'} — копия`,
    tags: [...source.tags],
    createdAt: now,
    updatedAt: now,
  };

  await invoke('create_secure_entry', { entry: copy });
  return copy;
}
