import { createSlice, type PayloadAction } from '@reduxjs/toolkit';
import { isTauri } from '@tauri-apps/api/core';

export type ThemePreference = 'system' | 'light' | 'dark' | 'cream';
export type Language = 'ru' | 'en';
export type EditorFontSize = 'small' | 'medium' | 'large';
export type EditorWidth = 'compact' | 'normal' | 'wide';

export type PreferencesState = {
  journalName: string;
  theme: ThemePreference;
  language: Language;
  firstDayOfWeek: 'monday' | 'sunday';
  timeFormat: '24h' | '12h';
  editorFontSize: EditorFontSize;
  editorWidth: EditorWidth;
  sidebarBackground: boolean;
  windowTransparency: boolean;
  autosave: boolean;
  autosaveDelay: number;
  showToolbar: boolean;
  spellcheck: boolean;
  showWordCount: boolean;
  showEntryTime: boolean;
};

export const PREFERENCES_STORAGE_KEY = 'maybeLater.preferences.v1';

const defaults: PreferencesState = {
  journalName: 'Мой дневник',
  theme: 'system',
  language: 'ru',
  firstDayOfWeek: 'monday',
  timeFormat: '24h',
  editorFontSize: 'medium',
  editorWidth: 'normal',
  sidebarBackground: true,
  windowTransparency: false,
  autosave: true,
  autosaveDelay: 500,
  showToolbar: true,
  spellcheck: true,
  showWordCount: true,
  showEntryTime: true,
};

function loadPreferences(): PreferencesState {
  try {
    const saved = JSON.parse(localStorage.getItem(PREFERENCES_STORAGE_KEY) ?? '{}');
    return { ...defaults, ...saved };
  } catch {
    return defaults;
  }
}

function persist(state: PreferencesState) {
  try {
    localStorage.setItem(PREFERENCES_STORAGE_KEY, JSON.stringify(state));
  } catch {
    // The application remains usable when localStorage is unavailable.
  }
}

const preferencesSlice = createSlice({
  name: 'preferences',
  initialState: loadPreferences(),
  reducers: {
    hydratePreferences(_state, action: PayloadAction<PreferencesState>) {
      return action.payload;
    },
    setJournalName(state, action: PayloadAction<string>) {
      const value = action.payload.trim();
      if (value) state.journalName = value;
      if (!isTauri()) persist(state);
    },
    setPreference<K extends keyof PreferencesState>(
      state: PreferencesState,
      action: PayloadAction<{ key: K; value: PreferencesState[K] }>,
    ) {
      Object.assign(state, { [action.payload.key]: action.payload.value });
      if (!isTauri()) persist(state);
    },
  },
});

export const { hydratePreferences, setJournalName, setPreference } = preferencesSlice.actions;
export default preferencesSlice.reducer;
