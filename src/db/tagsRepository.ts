import { isSqliteResetPending } from '../app/store';
import { invoke } from '@tauri-apps/api/core';
import type { Tag } from '../types/tag';


// Track tag mutations so backup/restore never reads a half-finished tag operation.
const pendingTagWrites = new Set<Promise<void>>();
function trackTagWrite(write: Promise<void>): Promise<void> {
  pendingTagWrites.add(write);
  // Register cleanup on both outcomes without creating an unhandled rejection.
  void write.then(() => pendingTagWrites.delete(write), () => pendingTagWrites.delete(write));
  return write;
}

export async function waitForSqliteTagWrites(): Promise<void> {
  while (pendingTagWrites.size > 0) {
    const results = await Promise.allSettled([...pendingTagWrites]);
    const failure = results.find((result): result is PromiseRejectedResult => result.status === 'rejected');
    if (failure) throw failure.reason;
  }
}


export async function loadTagsFromSqlite(): Promise<Tag[]> {
  return invoke<Tag[]>('load_secure_tags');
}

export function createTagInSqlite(tag: Tag): Promise<void> {
  if (isSqliteResetPending()) return Promise.reject(new Error('Сброс данных выполняется'));
  return trackTagWrite(createTagImpl(tag));
}
async function createTagImpl(tag: Tag): Promise<void> {
  await invoke('create_secure_tag', { tag });
}

/** SQLite triggers update entries.tags in the same transaction as this UPDATE. */
export function updateTagInSqlite(tag: Tag): Promise<void> {
  if (isSqliteResetPending()) return Promise.reject(new Error('Сброс данных выполняется'));
  return trackTagWrite(updateTagImpl(tag));
}
async function updateTagImpl(tag: Tag): Promise<void> {
  await invoke('update_secure_tag', { tag });
}

/** SQLite trigger removes the tag from entries atomically with DELETE. */
export function deleteTagInSqlite(id: string): Promise<void> {
  if (isSqliteResetPending()) return Promise.reject(new Error('Сброс данных выполняется'));
  return trackTagWrite(deleteTagImpl(id));
}
async function deleteTagImpl(id: string): Promise<void> {
  await invoke('delete_secure_tag', { id });
}
