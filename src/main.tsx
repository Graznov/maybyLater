import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { Provider } from 'react-redux';
import { store, enableSqliteReadOnly, beginSqliteLock, cancelSqliteLock, clearSqliteInMemoryCache, waitForAllSqliteWrites } from './app/store';
import { invoke } from '@tauri-apps/api/core';
import { waitForSqliteTagWrites } from './db/tagsRepository';
import { hydrateEntries, clearPrivateEntries } from './features/entries/entriesSlice';
import { isTauri } from '@tauri-apps/api/core';
import { loadEntriesFromSqlite } from './db/loadEntries';
import { loadTagsFromSqlite } from './db/tagsRepository';
import { hydrateTags, clearPrivateTags } from './features/tags/tagsSlice';
import App from './App';
import UnlockScreen from './UnlockScreen';
import { loadPreferencesFromSqlite } from './db/loadPreferences';
import { hydratePreferences, clearPrivatePreferences } from './features/preferences/preferencesSlice';
import { migrateCalendarColorsToSqlite } from './db/migrateCalendarColors';
import { hydrateCalendarColors, clearPrivateCalendarColors } from './features/calendar/calendarSlice';
import './index.css';

const root = createRoot(document.getElementById('root')!);

function renderApp() {
  root.render(
    <StrictMode>
      <Provider store={store}>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </Provider>
    </StrictMode>,
  );
}

async function bootstrap() {
  if (!isTauri()) {
    renderApp();
    return;
  }
  try {
    // Do not run the old migration on startup: it could copy stale localStorage
    // data into the database after SQLite becomes the source of truth.
    const entries = await loadEntriesFromSqlite();
    store.dispatch(hydrateEntries(entries));
    const tags = await loadTagsFromSqlite();
    store.dispatch(hydrateTags(tags));
    console.log(`✅ SQLite tags integration: ${tags.length} тегов загружено из SQLite`);
    const preferences = await loadPreferencesFromSqlite(store.getState().preferences);
    store.dispatch(hydratePreferences(preferences));
    const calendarColors = await migrateCalendarColorsToSqlite();
    store.dispatch(hydrateCalendarColors(calendarColors));
    enableSqliteReadOnly();
    // Keep the write gate raised throughout unlock and data hydration.
    cancelSqliteLock();
    console.log(`✅ SQLite integration: ${entries.length} записей загружено в Redux непосредственно из SQLite`);
    renderApp();
  } catch (error) {
    console.error('❌ SQLite integration: ошибка загрузки', error);
    // Fail closed. Do not silently fall back to a potentially stale localStorage copy.
    root.render(
      <div style={{padding:40,color:'#fff',background:'#172435',minHeight:'100vh',fontFamily:'sans-serif'}}>
        <h2>Не удалось подготовить данные SQLite</h2>
        <p>Дневник не открыт, чтобы избежать случайного изменения данных. Проверь консоль DevTools.</p>
        <p>localStorage и база SQLite не изменены.</p>
      </div>,
    );
  }
}
let lockRequestInProgress = false;

async function lockJournal(): Promise<void> {
  if (lockRequestInProgress) return;
  lockRequestInProgress = true;
  let backendLocked = false;
  try {
    beginSqliteLock();
    // Stop new writes first, then drain every frontend queue before Rust closes
    // the SQLCipher connection. Rust independently waits for active DB operations.
    await Promise.all([waitForAllSqliteWrites(), waitForSqliteTagWrites()]);
    await invoke('lock_secure_database');
    backendLocked = true;

    clearSqliteInMemoryCache();
    store.dispatch(clearPrivateEntries());
    store.dispatch(clearPrivateTags());
    store.dispatch(clearPrivatePreferences());
    store.dispatch(clearPrivateCalendarColors());

    root.render(<UnlockScreen onUnlocked={() => { void bootstrap(); }} />);
  } catch (error) {
    if (backendLocked) {
      // The database is already locked; never return to the journal UI.
      root.render(<UnlockScreen onUnlocked={() => { void bootstrap(); }} />);
    } else {
      cancelSqliteLock();
      window.alert(`Не удалось заблокировать дневник: ${String(error)}`);
    }
    console.error('Не удалось заблокировать дневник', error);
  } finally {
    lockRequestInProgress = false;
  }
}

if (isTauri()) {
  window.addEventListener('journal-lock-request', () => { void lockJournal(); });
}

if (isTauri()) {
  root.render(<UnlockScreen onUnlocked={() => { void bootstrap(); }} />);
} else {
  void bootstrap();
}
