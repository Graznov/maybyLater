import { invoke } from '@tauri-apps/api/core';
import type { Entry } from '../types/entry';

export async function loadEntriesFromSqlite(): Promise<Entry[]> {
  return invoke<Entry[]>('load_secure_entries');
}
