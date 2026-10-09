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
import './index.css';

const root = createRoot(document.getElementById('root')!);

function renderApp(sqliteMode: boolean) {
  root.render(
    <StrictMode>
      <Provider store={store}>
        <BrowserRouter>
          {sqliteMode && (
            <div style={{position:'fixed',top:38,right:14,zIndex:99999,padding:'8px 14px',
              background:'#283e58',color:'#fff',border:'1px solid #7190af',
              borderRadius:10,fontSize:12,pointerEvents:'none'}}>
              SQLite: создание, редактирование, дублирование и удаление включены; теги сохраняются в SQLite
            </div>
          )}
          <App />
        </BrowserRouter>
      </Provider>
    </StrictMode>,
  );
}

async function bootstrap() {
  if (!isTauri()) {
    renderApp(false);
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
    enableSqliteReadOnly();
    console.log(`✅ SQLite integration: ${entries.length} записей загружено в Redux непосредственно из SQLite`);
    console.log('🔒 SQLite integration: создание, редактирование, дублирование и удаление включены; операции с тегами включены; localStorage не изменён.');
    renderApp(true);
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
