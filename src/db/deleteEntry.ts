import { invoke } from '@tauri-apps/api/core';

/** Delete exactly one existing entry. The UI changes only after success. */
export async function deleteEntryInSqlite(id: string): Promise<void> {
  await invoke('delete_secure_entry', { id });
}
