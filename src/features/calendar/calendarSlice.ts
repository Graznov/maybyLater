import { createSlice, type PayloadAction } from '@reduxjs/toolkit';
import { isTauri } from '@tauri-apps/api/core';

const STORAGE_KEY = 'maybeLater.calendarColors.v1';

export type CalendarColors = Record<string, string>;

type CalendarState = {
  dateColors: CalendarColors;
};

function loadDateColors(): CalendarColors {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function persistDateColors(colors: CalendarColors) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(colors));
  } catch {
    // Keep the app usable if localStorage is unavailable.
  }
}

const initialState: CalendarState = {
  dateColors: loadDateColors(),
};

const calendarSlice = createSlice({
  name: 'calendar',
  initialState,
  reducers: {
    hydrateCalendarColors(state, action: PayloadAction<CalendarColors>) {
      state.dateColors = action.payload;
    },
    setDateColor(
      state,
      action: PayloadAction<{ date: string; color: string }>,
    ) {
      state.dateColors[action.payload.date] = action.payload.color;
      if (!isTauri()) persistDateColors(state.dateColors);
    },
    clearDateColor(state, action: PayloadAction<string>) {
      delete state.dateColors[action.payload];
      if (!isTauri()) persistDateColors(state.dateColors);
    },
  },
});

export const { setDateColor, clearDateColor, hydrateCalendarColors } = calendarSlice.actions;
export default calendarSlice.reducer;
