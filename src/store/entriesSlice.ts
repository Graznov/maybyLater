import {
  createSlice,
  type PayloadAction,
} from '@reduxjs/toolkit';

import type { Entry } from '../types/entry';

type State = {
  items: Entry[];
  selectedId: string | null;
  selectedDate: string;
};

const getToday = () => {
  const now = new Date();

  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');

  return `${year}-${month}-${day}`;
};

const initialState: State = {
  items: [],
  selectedId: null,
  selectedDate: getToday(),
};

const slice = createSlice({
  name: 'entries',

  initialState,

  reducers: {
    selectEntry: (
      state,
      action: PayloadAction<string>,
    ) => {
      state.selectedId = action.payload;

      const entry = state.items.find(
        (item) => item.id === action.payload,
      );

      if (entry) {
        state.selectedDate = entry.date;
      }
    },

    selectDate: (
      state,
      action: PayloadAction<string>,
    ) => {
      state.selectedDate = action.payload;

      const entry = state.items.find(
        (item) => item.date === action.payload,
      );

      state.selectedId = entry?.id ?? null;
    },

    updateEntry: (
      state,
      action: PayloadAction<Entry>,
    ) => {
      const index = state.items.findIndex(
        (item) => item.id === action.payload.id,
      );

      if (index >= 0) {
        state.items[index] = action.payload;
      }
    },
  },
});

export const {
  selectEntry,
  selectDate,
  updateEntry,
} = slice.actions;

export default slice.reducer;