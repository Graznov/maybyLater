import { invoke } from '@tauri-apps/api/core';
import type { PreferencesState } from '../features/preferences/preferencesSlice';

/** One complete snapshot per change; callers serialize writes to prevent stale overwrites. */
export async function savePreferencesToSqlite(preferences: PreferencesState): Promise<void> {
  await invoke("save_secure_preferences", { value: JSON.stringify(preferences) });
}
