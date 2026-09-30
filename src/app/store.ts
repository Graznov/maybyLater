import { configureStore } from '@reduxjs/toolkit';
import entries from '../store/entriesSlice';
export const store=configureStore({reducer:{entries}});
export type RootState=ReturnType<typeof store.getState>; export type AppDispatch=typeof store.dispatch;
