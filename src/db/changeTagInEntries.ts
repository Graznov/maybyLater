import { invoke } from '@tauri-apps/api/core';
import type { Entry } from '../types/entry';

/** Atomically change the tags of all affected entries through SecureSession. */
export async function changeTagInEntries(
  entries: Entry[], oldName: string, newName: string | null,
): Promise<number> {
  const affected = entries.filter(entry => entry.tags.includes(oldName));
  if (affected.length === 0) return 0;

  const replacements: [string, string[]][] = affected.map(entry => [
    entry.id,
    newName === null
      ? entry.tags.filter(tag => tag !== oldName)
      : entry.tags.map(tag => tag === oldName ? newName : tag),
  ]);

  const changed = await invoke<number>('change_secure_entry_tags', {
    replacements,
    updatedAt: new Date().toISOString(),
  });

  if (changed !== replacements.length) {
    throw new Error(
      `Изменено ${changed} из ${replacements.length} записей. Перезагрузите приложение перед продолжением.`,
    );
  }

  return changed;
}
