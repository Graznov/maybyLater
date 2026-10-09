import { useState } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { createTagInSqlite, updateTagInSqlite, deleteTagInSqlite, loadTagsFromSqlite } from '../db/tagsRepository';
import { waitForSqliteEntryWrites } from '../app/store';
import { Modal } from '../components/Modal/Modal';
import { Check, Pencil, Plus, Trash2, X } from 'lucide-react';
import { useAppDispatch, useAppSelector } from '../app/hooks';
import {
  removeTagFromEntries,
  renameTagInEntries,
  hydrateEntries,
  setTag,
} from '../features/entries/entriesSlice';
import { addTag, deleteTag, updateTag, hydrateTags } from '../features/tags/tagsSlice';
import './TagsPage.css';
import { loadEntriesFromSqlite } from '../db/loadEntries';

type Draft = {
  name: string;
  color: string;
};

const DEFAULT_COLOR = '#7eb5dc';

export function TagsPage() {
  const dispatch = useAppDispatch();
  const tags = useAppSelector((state) => state.tags.items);
  const entries = useAppSelector((state) => state.entries.items);
  const [tagOperationPending, setTagOperationPending] = useState(false);
  const [newTag, setNewTag] = useState<Draft>({ name: '', color: DEFAULT_COLOR });
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft>({ name: '', color: DEFAULT_COLOR });
  const [tagToDelete, setTagToDelete] = useState<{ id: string; name: string } | null>(null);

  const normalizedNames = tags.map((tag) => tag.name.toLocaleLowerCase());

  const createTag = async () => {
    if (tagOperationPending) return;
    const name = newTag.name.trim();
    if (!name || normalizedNames.includes(name.toLocaleLowerCase())) return;
    if (!isTauri()) {
      dispatch(addTag(name, newTag.color));
      setNewTag({ name: '', color: DEFAULT_COLOR });
      return;
    }
    setTagOperationPending(true);
    try {
      const tag = { id: crypto.randomUUID(), name, color: newTag.color };
      await createTagInSqlite(tag);
      dispatch(hydrateTags(await loadTagsFromSqlite()));
      setNewTag({ name: '', color: DEFAULT_COLOR });
      console.log('✅ SQLite tags: тег создан:', name);
    } catch (error) {
      console.error('❌ SQLite tags: создание не выполнено', error);
      window.dispatchEvent(new CustomEvent('sqlite-save-error', { detail: String(error) }));
    } finally { setTagOperationPending(false); }
  };

  const startEditing = (id: string, name: string, color: string) => {
    setEditingId(id);
    setDraft({ name, color });
  };

  const saveTag = async (id: string, oldName: string) => {
    if (tagOperationPending) return;
    const name = draft.name.trim();
    const duplicate = tags.some(tag => tag.id !== id && tag.name.toLocaleLowerCase() === name.toLocaleLowerCase());
    if (!name || duplicate) return;
    if (!isTauri()) {
      dispatch(updateTag({ id, name, color: draft.color }));
      if (oldName !== name) dispatch(renameTagInEntries({ oldName, newName: name }));
      setEditingId(null);
      return;
    }
    setTagOperationPending(true);
    try {
      await waitForSqliteEntryWrites();
      await updateTagInSqlite({ id, name, color: draft.color });
      dispatch(hydrateTags(await loadTagsFromSqlite()));
      dispatch(hydrateEntries(await loadEntriesFromSqlite()));
      dispatch(setTag(null));
      setEditingId(null);
      console.log('✅ SQLite tags: тег обновлён:', name);
    } catch (error) {
      console.error('❌ SQLite tags: переименование не выполнено', error);
      window.dispatchEvent(new CustomEvent('sqlite-save-error', { detail: String(error) }));
    } finally {
      setTagOperationPending(false);
    }
  };

  const removeTag = async () => {
    if (!tagToDelete || tagOperationPending) return;
    if (!isTauri()) {
      dispatch(deleteTag(tagToDelete.id));
      dispatch(removeTagFromEntries(tagToDelete.name));
      setTagToDelete(null);
      return;
    }
    setTagOperationPending(true);
    try {
      await waitForSqliteEntryWrites();
      await deleteTagInSqlite(tagToDelete.id);
      dispatch(hydrateTags(await loadTagsFromSqlite()));
      dispatch(hydrateEntries(await loadEntriesFromSqlite()));
      dispatch(setTag(null));
      setTagToDelete(null);
      console.log('✅ SQLite tags: тег удалён');
    } catch (error) {
      console.error('❌ SQLite tags: удаление не выполнено', error);
      window.dispatchEvent(new CustomEvent('sqlite-save-error', { detail: String(error) }));
    } finally {
      setTagOperationPending(false);
    }
  };

  return (
    <main className="tags-page">
      <section className="tags-page__panel">
        <header className="tags-page__header">
          <div>
            <h1>Теги</h1>
            <p>Создавайте свои категории и выбирайте для них цвет.</p>
          </div>
        </header>

        <div className="tag-create">
          <label className="tag-color" title="Цвет нового тега">
            <input
              type="color"
              value={newTag.color}
              onChange={(event) =>
                setNewTag((current) => ({ ...current, color: event.target.value }))
              }
            />
            <span style={{ background: newTag.color }} />
          </label>

          <input
            value={newTag.name}
            onChange={(event) =>
              setNewTag((current) => ({ ...current, name: event.target.value }))
            }
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                createTag();
              }
            }}
            placeholder="Название нового тега"
          />

          <button className="tag-primary" onClick={createTag} disabled={!newTag.name.trim() || tagOperationPending}>
            <Plus size={18} />
            Добавить
          </button>
        </div>

        <div className="tag-list">
          {tags.map((tag) => {
            const isEditing = editingId === tag.id;
            const usageCount = entries.filter((entry) => entry.tags.includes(tag.name)).length;

            return (
              <div className="tag-row" key={tag.id}>
                {isEditing ? (
                  <>
                    <label className="tag-color" title="Изменить цвет">
                      <input
                        type="color"
                        value={draft.color}
                        onChange={(event) =>
                          setDraft((current) => ({ ...current, color: event.target.value }))
                        }
                      />
                      <span style={{ background: draft.color }} />
                    </label>

                    <input
                      className="tag-row__input"
                      value={draft.name}
                      autoFocus
                      onChange={(event) =>
                        setDraft((current) => ({ ...current, name: event.target.value }))
                      }
                      onKeyDown={(event) => {
                        if (event.key === 'Enter') {
                          saveTag(tag.id, tag.name);
                        }

                        if (event.key === 'Escape') {
                          setEditingId(null);
                        }
                      }}
                    />

                    <div className="tag-row__actions">
                      <button
                        title="Сохранить"
                        onClick={() => saveTag(tag.id, tag.name)}
                        disabled={tagOperationPending}
                      >
                        <Check size={18} />
                      </button>
                      <button title="Отмена" onClick={() => setEditingId(null)}>
                        <X size={18} />
                      </button>
                    </div>
                  </>
                ) : (
                  <>
                    <span className="tag-row__dot" style={{ background: tag.color }} />
                    <div className="tag-row__info">
                      <strong>{tag.name}</strong>
                      <span>
                        {usageCount === 0
                          ? 'Нет записей'
                          : `Записей: ${usageCount}`}
                      </span>
                    </div>
                    <div className="tag-row__actions">
                      <button
                        title="Редактировать"
                        onClick={() => startEditing(tag.id, tag.name, tag.color)}
                      >
                        <Pencil size={17} />
                      </button>
                      <button
                        className="tag-row__delete"
                        title="Удалить"
                        onClick={() => setTagToDelete({ id: tag.id, name: tag.name })}
                      >
                        <Trash2 size={17} />
                      </button>
                    </div>
                  </>
                )}
              </div>
            );
          })}
        </div>
      </section>

      <Modal
        open={Boolean(tagToDelete)}
        title="Удалить тег?"
        danger
        onClose={() => setTagToDelete(null)}
        footer={
          <>
            <button
              className="modal__button"
              onClick={() => setTagToDelete(null)}
            >
              Отмена
            </button>
            <button
              className="modal__button modal__button--danger"
              onClick={removeTag}
              disabled={tagOperationPending}
            >
              Удалить
            </button>
          </>
        }
      >
        <p>
          Тег «{tagToDelete?.name}» будет удалён из всех записей.
          Это действие нельзя отменить.
        </p>
      </Modal>
    </main>
  );
}
