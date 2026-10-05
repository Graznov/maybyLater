import { createSlice,type PayloadAction } from '@reduxjs/toolkit'; import type { Entry } from '../../types/entry'; import { seedEntries } from '../../data/seed';
const KEY='maybeLater.entries.v1';
function load():Entry[]{try{const raw=localStorage.getItem(KEY);return raw?JSON.parse(raw):seedEntries}catch{return seedEntries}}
function persist(items:Entry[]){try{localStorage.setItem(KEY,JSON.stringify(items))}catch{}}
type State={items:Entry[];selectedDate:string;selectedId:string|null;query:string;tag:string|null};
const today=new Date().toISOString().slice(0,10); const initialItems=load();
const initialState:State={items:initialItems,selectedDate:initialItems[0]?.date??today,selectedId:initialItems[0]?.id??null,query:'',tag:null};
const slice=createSlice({name:'entries',initialState,reducers:{
 selectDate(s,a:PayloadAction<string>){s.selectedDate=a.payload; const first=s.items.find(e=>e.date===a.payload);s.selectedId=first?.id??null},
 selectEntry(s,a:PayloadAction<string>){s.selectedId=a.payload;const e=s.items.find(x=>x.id===a.payload);if(e)s.selectedDate=e.date},
 addEntry(s){const now=new Date().toISOString();const e:Entry={id:crypto.randomUUID(),title:'Новая запись',content:'',date:s.selectedDate,tags:[],createdAt:now,updatedAt:now};s.items.unshift(e);s.selectedId=e.id;persist(s.items)},
 updateEntry(s,a:PayloadAction<{id:string;patch:Partial<Pick<Entry,'title'|'content'|'tags'|'date'>>}>){const e=s.items.find(x=>x.id===a.payload.id);if(e){Object.assign(e,a.payload.patch);e.updatedAt=new Date().toISOString();persist(s.items)}},
 deleteEntry(s,a:PayloadAction<string>){s.items=s.items.filter(e=>e.id!==a.payload);if(s.selectedId===a.payload)s.selectedId=s.items.find(e=>e.date===s.selectedDate)?.id??null;persist(s.items)},
 setQuery(s,a:PayloadAction<string>){s.query=a.payload},setTag(s,a:PayloadAction<string|null>){s.tag=a.payload}
}}); export const {selectDate,selectEntry,addEntry,updateEntry,deleteEntry,setQuery,setTag}=slice.actions; export default slice.reducer;