import { invoke } from '@tauri-apps/api/core';
import type { Entry } from '../types/entry';

/** Write a complete snapshot so a serialized queue cannot reorder edits. */
export async function updateEntryInSqlite(entry: Entry): Promise<void> {
  await invoke('update_secure_entry', { entry });
}
