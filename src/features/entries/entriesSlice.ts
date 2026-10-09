import { createSlice, type PayloadAction } from '@reduxjs/toolkit';
import type { Entry } from '../../types/entry';

const STORAGE_KEY = 'maybeLater.entries.v1';

function loadEntries(): Entry[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === null) return [];
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as Entry[]) : [];
  } catch {
    return [];
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

function getLocalToday(): string {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

const today = getLocalToday();
const initialItems = loadEntries();
const todayEntry = initialItems.find((entry) => entry.date === today);

const initialState: EntriesState = {
  items: initialItems,
  selectedDate: today,
  selectedId: todayEntry?.id ?? null,
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
    duplicateEntry(state, action: PayloadAction<string>) {
      const source = state.items.find((entry) => entry.id === action.payload);
      if (!source) return;
      const now = new Date().toISOString();
      const copy: Entry = {
        ...source,
        id: crypto.randomUUID(),
        title: `${source.title || 'Без названия'} — копия`,
        createdAt: now,
        updatedAt: now,
        tags: [...source.tags],
      };
      state.items.unshift(copy);
      state.selectedId = copy.id;
      state.selectedDate = copy.date;
      persistEntries(state.items);
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
  duplicateEntry,
  deleteEntry,
  renameTagInEntries,
  removeTagFromEntries,
  setQuery,
  setTag,
} = entriesSlice.actions;

export default entriesSlice.reducer;
