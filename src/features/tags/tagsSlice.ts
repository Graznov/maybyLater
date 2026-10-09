import { isTauri } from '@tauri-apps/api/core';
import { createSlice, type PayloadAction } from '@reduxjs/toolkit';
import { seedTags } from '../../data/tags';
import type { Tag } from '../../types/tag';

const STORAGE_KEY = 'maybeLater.tags.v1';

function loadTags(): Tag[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : seedTags;
  } catch {
    return seedTags;
  }
}

function persistTags(tags: Tag[]) {
  try {
    if (!isTauri()) localStorage.setItem(STORAGE_KEY, JSON.stringify(tags));
  } catch {
    // localStorage can be unavailable in private/restricted contexts.
  }
}

type TagsState = {
  items: Tag[];
};

const initialState: TagsState = {
  items: loadTags(),
};

const tagsSlice = createSlice({
  name: 'tags',
  initialState,
  reducers: {
    hydrateTags(state, action: PayloadAction<Tag[]>) { state.items = action.payload; },
    addTag: {
      reducer(state, action: PayloadAction<Tag>) {
        state.items.push(action.payload);
        persistTags(state.items);
      },
      prepare(name: string, color: string) {
        return {
          payload: {
            id: crypto.randomUUID(),
            name: name.trim(),
            color,
          },
        };
      },
    },
    updateTag(
      state,
      action: PayloadAction<{ id: string; name: string; color: string }>,
    ) {
      const tag = state.items.find((item) => item.id === action.payload.id);

      if (!tag) {
        return;
      }

      tag.name = action.payload.name.trim();
      tag.color = action.payload.color;
      persistTags(state.items);
    },
    deleteTag(state, action: PayloadAction<string>) {
      state.items = state.items.filter((tag) => tag.id !== action.payload);
      persistTags(state.items);
    },
  },
});

export const { hydrateTags, addTag, updateTag, deleteTag } = tagsSlice.actions;
export default tagsSlice.reducer;
