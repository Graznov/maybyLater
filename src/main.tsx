import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { Provider } from 'react-redux';
import { store, enableSqliteReadOnly } from './app/store';
import { hydrateEntries } from './features/entries/entriesSlice';
import { isTauri } from '@tauri-apps/api/core';
import { loadEntriesFromSqlite } from './db/loadEntries';
import { loadTagsFromSqlite } from './db/tagsRepository';
import { hydrateTags } from './features/tags/tagsSlice';
import App from './App';
import { loadPreferencesFromSqlite } from './db/loadPreferences';
import { hydratePreferences } from './features/preferences/preferencesSlice';
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
    enableSqliteReadOnly();
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
void bootstrap();
