import { createSlice, type PayloadAction } from '@reduxjs/toolkit';
import { mockEntries } from '../data/mockEntries';
import type { Entry } from '../types/entry';
type State={items:Entry[];selectedId:string|null;selectedDate:string};
const initialState:State={items:mockEntries,selectedId:'1',selectedDate:'2026-09-23'};
const slice=createSlice({name:'entries',initialState,reducers:{
 selectEntry:(s,a:PayloadAction<string>)=>{s.selectedId=a.payload; const e=s.items.find(x=>x.id===a.payload); if(e)s.selectedDate=e.date},
 selectDate:(s,a:PayloadAction<string>)=>{s.selectedDate=a.payload; const e=s.items.find(x=>x.date===a.payload); if(e)s.selectedId=e.id},
 updateEntry:(s,a:PayloadAction<Entry>)=>{const i=s.items.findIndex(x=>x.id===a.payload.id);if(i>=0)s.items[i]=a.payload}
}});
export const {selectEntry,selectDate,updateEntry}=slice.actions; export default slice.reducer;
