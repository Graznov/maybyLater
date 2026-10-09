import { configureStore, type Middleware } from '@reduxjs/toolkit';
import { isTauri } from '@tauri-apps/api/core';
import { sqliteEntryCreated, sqliteEntryUpdated, sqliteEntryDeleted } from '../features/entries/entriesSlice';
import { createEntryInSqlite } from '../db/createEntry';
import { updateEntryInSqlite } from '../db/updateEntry';
import { duplicateEntryInSqlite } from '../db/duplicateEntry';
import { deleteEntryInSqlite } from '../db/deleteEntry';
import { loadEntriesFromSqlite } from '../db/loadEntries';
import type { Entry } from '../types/entry';
import { savePreferencesToSqlite } from '../db/savePreferences';
import { saveCalendarColor } from '../db/saveCalendarColor';
import entriesReducer from '../features/entries/entriesSlice';
import tagsReducer from '../features/tags/tagsSlice';
import calendarReducer from '../features/calendar/calendarSlice';
import preferencesReducer from '../features/preferences/preferencesSlice';

// Temporary SQLite verification mode: Redux actions that modify diary entries are blocked.
// The UI remains navigable, but editing must not silently write only to localStorage.
let sqliteReadOnly = false;
let creatingEntry = false;
let updateQueue: Promise<void> = Promise.resolve();
let updateFailed = false;
let destructiveOperationPending = false;
const latestEntries = new Map<string, Entry>();
export function sqliteUpdateHasFailed(): boolean { return updateFailed; }
export async function waitForSqliteEntryWrites(): Promise<void> {
  if (creatingEntry || destructiveOperationPending) throw new Error('Дождитесь завершения операции с записью');
  await updateQueue;
  if (updateFailed) throw new Error('Предыдущая запись в SQLite завершилась ошибкой');
}

function reportUpdateError(error: unknown) {
  console.error('❌ SQLite update: сохранение не выполнено; редактирование заблокировано', error);
  window.dispatchEvent(new CustomEvent('sqlite-save-error', { detail: String(error) }));
}

export function enableSqliteReadOnly(): void { sqliteReadOnly = true; }
const blockedEntryActions = new Set([
  'entries/addEntry', 'entries/updateEntry', 'entries/duplicateEntry',
  'entries/deleteEntry', 'entries/renameTagInEntries', 'entries/removeTagFromEntries',
]);
const readOnlyGuard: Middleware = () => next => action => {
  if (sqliteReadOnly && typeof action === 'object' && action !== null && 'type' in action
      && typeof action.type === 'string' && blockedEntryActions.has(action.type)) {
    if (action.type === 'entries/addEntry' && isTauri()) {
      if (creatingEntry) return action;
      creatingEntry = true;
      const date = (store.getState() as RootState).entries.selectedDate;
      void createEntryInSqlite(date)
        .then(entry => {
          store.dispatch(sqliteEntryCreated(entry));
          console.log('✅ SQLite create: запись сохранена в SQLite:', entry.id);
        })
        .catch(error => {
          console.error('❌ SQLite create: запись не создана', error);
          store.dispatch({ type: 'sqliteCreate/failed', payload: String(error) });
        })
        .finally(() => { creatingEntry = false; });
      return action;
    }
    if (action.type === 'entries/updateEntry' && isTauri()) {
      if (updateFailed || destructiveOperationPending) return action;
      const payload = (action as { payload?: { id?: string; patch?: Partial<Pick<Entry, 'title' | 'content' | 'tags' | 'date'>> } }).payload;
      if (!payload?.id || !payload.patch) return action;
      const current = latestEntries.get(payload.id) ?? (store.getState() as RootState).entries.items.find(item => item.id === payload.id);
      if (!current) return action;
      const snapshot: Entry = { ...current, ...payload.patch, updatedAt: new Date().toISOString() };
      latestEntries.set(snapshot.id, snapshot);
      // Optimistic UI keeps the title field and rich-text editor responsive.
      // Database writes are serialized; failures force a reload from SQLite.
      store.dispatch(sqliteEntryUpdated(snapshot));
      updateQueue = updateQueue.then(async () => {
        if (updateFailed) return;
        try {
          await updateEntryInSqlite(snapshot);
          console.log('✅ SQLite update: запись сохранена:', snapshot.id);
        } catch (error) {
          updateFailed = true;
          reportUpdateError(error);
          try {
            const entries = await loadEntriesFromSqlite();
            store.dispatch({ type: 'entries/hydrateEntries', payload: entries });
          } catch (reloadError) {
            console.error('❌ SQLite update: не удалось восстановить данные из SQLite', reloadError);
          }
        }
      });
      return action;
    }
    if ((action.type === 'entries/duplicateEntry' || action.type === 'entries/deleteEntry') && isTauri()) {
      if (updateFailed || destructiveOperationPending || creatingEntry) {
        console.warn('⏳ SQLite: операция недоступна, пока выполняется другая операция или произошла ошибка сохранения');
        return action;
      }
      const id = (action as { payload?: unknown }).payload;
      if (typeof id !== 'string') return action;
      const existing = (store.getState() as RootState).entries.items.find(entry => entry.id === id);
      if (!existing) return action;
      destructiveOperationPending = true;
      // Wait for all previously queued editor updates before copying/deleting.
      // A failed update must not be followed by a destructive operation.
      const operation = action.type;
      updateQueue = updateQueue.then(async () => {
        if (updateFailed) return;
        if (operation === 'entries/duplicateEntry') {
          const source = (store.getState() as RootState).entries.items.find(entry => entry.id === id);
          if (!source) return;
          const copy = await duplicateEntryInSqlite(source);
          store.dispatch(sqliteEntryCreated(copy));
          console.log('✅ SQLite duplicate: копия создана:', copy.id);
        } else {
          await deleteEntryInSqlite(id);
          latestEntries.delete(id);
          store.dispatch(sqliteEntryDeleted(id));
          console.log('✅ SQLite delete: запись удалена:', id);
        }
      }).catch(error => {
        console.error(`❌ SQLite ${operation === 'entries/duplicateEntry' ? 'duplicate' : 'delete'}: операция не выполнена`, error);
        window.dispatchEvent(new CustomEvent('sqlite-save-error', { detail: String(error) }));
      }).finally(() => { destructiveOperationPending = false; });
      return action;
    }
    console.warn('🔒 SQLite read-only: изменение записи заблокировано:', action.type);
    return action;
  }
  return next(action);
};

// Serialize preference writes: a slow earlier save must never overwrite a newer value.
let preferencesWriteQueue: Promise<void> = Promise.resolve();
let preferencesWriteFailed = false;
export async function waitForSqlitePreferencesWrites(): Promise<void> {
  await preferencesWriteQueue;
  if (preferencesWriteFailed) throw new Error('Сохранение настроек SQLite завершилось ошибкой');
}
const sqlitePreferencesWriter: Middleware = api => next => action => {
  if (!isTauri() || typeof action !== 'object' || action === null || !('type' in action) ||
      (action.type !== 'preferences/setPreference' && action.type !== 'preferences/setJournalName')) return next(action);
  if (preferencesWriteFailed) {
    console.error('❌ SQLite preferences: изменение заблокировано после ошибки сохранения');
    return action;
  }
  const result = next(action);
  const snapshot = { ...(api.getState() as RootState).preferences };
  preferencesWriteQueue = preferencesWriteQueue.then(async () => {
    if (preferencesWriteFailed) return;
    try {
      await savePreferencesToSqlite(snapshot);
      console.log('✅ SQLite preferences write: 15 полей сохранено в SQLite');
    } catch (error) {
      preferencesWriteFailed = true;
      console.error('❌ SQLite preferences write: сохранение не выполнено', error);
      window.dispatchEvent(new CustomEvent('sqlite-save-error', { detail: `Настройки SQLite: ${String(error)}` }));
      // Avoid leaving unsaved optimistic settings in Redux; refresh from SQLite on restart.
      window.alert('Не удалось сохранить настройки в SQLite. Дальнейшие изменения заблокированы. Перезапустите приложение после устранения ошибки.');
    }
  });
  return result;
};

// Calendar color writes are serialized to preserve action order (including deletes).
let calendarWriteQueue: Promise<void> = Promise.resolve();
let calendarWriteFailed = false;
export async function waitForSqliteCalendarWrites(): Promise<void> {
  await calendarWriteQueue;
  if (calendarWriteFailed) throw new Error('Сохранение цветов календаря SQLite завершилось ошибкой');
}
const sqliteCalendarWriter: Middleware = () => next => action => {
  if (!isTauri() || typeof action !== 'object' || action === null || !('type' in action) ||
      (action.type !== 'calendar/setDateColor' && action.type !== 'calendar/clearDateColor')) return next(action);
  if (calendarWriteFailed) {
    console.error('❌ SQLite calendar: изменение заблокировано после ошибки сохранения');
    return action;
  }
  const payload = (action as { payload?: unknown }).payload;
  let date: string;
  let color: string | null;
  if (action.type === 'calendar/setDateColor') {
    const value = payload as { date?: string; color?: string } | undefined;
    if (typeof value?.date !== 'string' || typeof value.color !== 'string') return action;
    date = value.date;
    color = value.color;
  } else {
    if (typeof payload !== 'string') return action;
    date = payload;
    color = null;
  }
  const result = next(action);
  calendarWriteQueue = calendarWriteQueue.then(async () => {
    if (calendarWriteFailed) return;
    try {
      await saveCalendarColor(date, color);
      console.log(`✅ SQLite calendar colors write: ${date} ${color === null ? 'удалён' : 'сохранён'}`);
    } catch (error) {
      calendarWriteFailed = true;
      console.error('❌ SQLite calendar colors write: сохранение не выполнено', error);
      window.dispatchEvent(new CustomEvent('sqlite-save-error', { detail: `Календарь SQLite: ${String(error)}` }));
      window.alert('Не удалось сохранить цвет даты в SQLite. Изменения заблокированы до перезапуска приложения.');
    }
  });
  return result;
};

export const store = configureStore({
  middleware: getDefaultMiddleware => getDefaultMiddleware().concat(readOnlyGuard, sqlitePreferencesWriter, sqliteCalendarWriter),
  reducer: {
    entries: entriesReducer,
    tags: tagsReducer,
    calendar: calendarReducer,
    preferences: preferencesReducer,
  },
});

export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;
