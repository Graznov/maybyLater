import { createSlice, type PayloadAction } from '@reduxjs/toolkit';

const STORAGE_KEY = 'maybeLater.calendarColors.v1';

type CalendarColors = Record<string, string>;

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
    setDateColor(
      state,
      action: PayloadAction<{ date: string; color: string }>,
    ) {
      state.dateColors[action.payload.date] = action.payload.color;
      persistDateColors(state.dateColors);
    },
    clearDateColor(state, action: PayloadAction<string>) {
      delete state.dateColors[action.payload];
      persistDateColors(state.dateColors);
    },
  },
});

export const { setDateColor, clearDateColor } = calendarSlice.actions;
export default calendarSlice.reducer;
