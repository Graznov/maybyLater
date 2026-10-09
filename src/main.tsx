import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { Provider } from 'react-redux';
import { store } from './app/store';
import App from './App';
import './index.css';
import { isTauri } from '@tauri-apps/api/core';
import { testSqliteConnection } from './db/testConnection';

// Temporary SQLite diagnostic. Does not change diary data.
if (isTauri()) {
  void testSqliteConnection();
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Provider store={store}>
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </Provider>
  </StrictMode>,
);
