import { createSlice, type PayloadAction } from '@reduxjs/toolkit';
import type { Entry } from '../../types/entry';
import { seedEntries } from '../../data/seed';

const STORAGE_KEY = 'maybeLater.entries.v1';

function loadEntries(): Entry[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : seedEntries;
  } catch {
    return seedEntries;
  }
}

function persistEntries(items: Entry[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(items));
  } catch {
    // Ignore storage errors and keep the current session usable.
  }
}

type EntriesState = {
  items: Entry[];
  selectedDate: string;
  selectedId: string | null;
  query: string;
  tag: string | null;
};

const today = new Date().toISOString().slice(0, 10);
const initialItems = loadEntries();

const initialState: EntriesState = {
  items: initialItems,
  selectedDate: initialItems[0]?.date ?? today,
  selectedId: initialItems[0]?.id ?? null,
  query: '',
  tag: null,
};

const entriesSlice = createSlice({
  name: 'entries',
  initialState,
  reducers: {
    selectDate(state, action: PayloadAction<string>) {
      state.selectedDate = action.payload;
      const first = state.items.find((entry) => entry.date === action.payload);
      state.selectedId = first?.id ?? null;
    },
    selectEntry(state, action: PayloadAction<string>) {
      state.selectedId = action.payload;
      const entry = state.items.find((item) => item.id === action.payload);

      if (entry) {
        state.selectedDate = entry.date;
      }
    },
    addEntry(state) {
      const now = new Date().toISOString();
      const entry: Entry = {
        id: crypto.randomUUID(),
        title: 'Новая запись',
        content: '',
        date: state.selectedDate,
        tags: [],
        createdAt: now,
        updatedAt: now,
      };

      state.items.unshift(entry);
      state.selectedId = entry.id;
      persistEntries(state.items);
    },
    updateEntry(
      state,
      action: PayloadAction<{
        id: string;
        patch: Partial<Pick<Entry, 'title' | 'content' | 'tags' | 'date'>>;
      }>,
    ) {
      const entry = state.items.find((item) => item.id === action.payload.id);

      if (entry) {
        Object.assign(entry, action.payload.patch);
        entry.updatedAt = new Date().toISOString();
        persistEntries(state.items);
      }
    },
    deleteEntry(state, action: PayloadAction<string>) {
      state.items = state.items.filter((entry) => entry.id !== action.payload);

      if (state.selectedId === action.payload) {
        state.selectedId =
          state.items.find((entry) => entry.date === state.selectedDate)?.id ?? null;
      }

      persistEntries(state.items);
    },
    renameTagInEntries(
      state,
      action: PayloadAction<{ oldName: string; newName: string }>,
    ) {
      state.items.forEach((entry) => {
        entry.tags = entry.tags.map((tag) =>
          tag === action.payload.oldName ? action.payload.newName : tag,
        );
      });

      if (state.tag === action.payload.oldName) {
        state.tag = action.payload.newName;
      }

      persistEntries(state.items);
    },
    removeTagFromEntries(state, action: PayloadAction<string>) {
      state.items.forEach((entry) => {
        entry.tags = entry.tags.filter((tag) => tag !== action.payload);
      });

      if (state.tag === action.payload) {
        state.tag = null;
      }

      persistEntries(state.items);
    },
    setQuery(state, action: PayloadAction<string>) {
      state.query = action.payload;
    },
    setTag(state, action: PayloadAction<string | null>) {
      state.tag = action.payload;
    },
  },
});

export const {
  selectDate,
  selectEntry,
  addEntry,
  updateEntry,
  deleteEntry,
  renameTagInEntries,
  removeTagFromEntries,
  setQuery,
  setTag,
} = entriesSlice.actions;

export default entriesSlice.reducer;
