import { configureStore } from '@reduxjs/toolkit';
import entriesReducer from '../features/entries/entriesSlice';
import tagsReducer from '../features/tags/tagsSlice';
import calendarReducer from '../features/calendar/calendarSlice';
import preferencesReducer from '../features/preferences/preferencesSlice';

export const store = configureStore({
  reducer: {
    entries: entriesReducer,
    tags: tagsReducer,
    calendar: calendarReducer,
    preferences: preferencesReducer,
  },
});

export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;
