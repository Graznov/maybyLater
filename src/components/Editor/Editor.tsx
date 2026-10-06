import { useEffect, useRef, useState } from 'react';
import { Check, Copy, Download, Info, MoreHorizontal, Trash2 } from 'lucide-react';
import { format } from 'date-fns';
import { ru } from 'date-fns/locale';
import { EditorContent, useEditor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import TaskList from '@tiptap/extension-task-list';
import TaskItem from '@tiptap/extension-task-item';
import TextAlign from '@tiptap/extension-text-align';
import { Color, TextStyle } from '@tiptap/extension-text-style';
import { useAppDispatch, useAppSelector } from '../../app/hooks';
import { deleteEntry, duplicateEntry, updateEntry } from '../../features/entries/entriesSlice';
import { EditorToolbar } from './EditorToolbar';
import { Modal } from '../Modal/Modal';
import { ThemeToggle } from '../ThemeToggle/ThemeToggle';
import './Editor.css';

export function Editor() {
  const dispatch = useAppDispatch();
  const state = useAppSelector((store) => store.entries);
  const availableTags = useAppSelector((store) => store.tags.items);
  const preferences = useAppSelector((store) => store.preferences);
  const entry = state.items.find((item) => item.id === state.selectedId);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [infoOpen, setInfoOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  const tiptap = useEditor({
    extensions: [
      StarterKit.configure({
        heading: { levels: [1, 2, 3] },
        link: { openOnClick: false, autolink: true, defaultProtocol: 'https' },
      }),
      TaskList,
      TaskItem.configure({ nested: true }),
      TextStyle,
      Color,
      TextAlign.configure({ types: ['heading', 'paragraph'] }),
    ],
    content: entry?.content ?? '',
    editorProps: {
      attributes: {
        class: 'editor__richtext tiptap',
        spellcheck: preferences.spellcheck ? 'true' : 'false',
      },
    },
    onUpdate: ({ editor }) => {
      if (!entry) return;
      dispatch(updateEntry({ id: entry.id, patch: { content: editor.getHTML() } }));
    },
  });

  useEffect(() => {
    if (!tiptap || !entry) return;
    if (tiptap.getHTML() !== entry.content) {
      tiptap.commands.setContent(entry.content || '', { emitUpdate: false });
    }
  }, [entry?.id, tiptap]);

  useEffect(() => {
    if (!tiptap) return;
    tiptap.setOptions({
      editorProps: {
        attributes: {
          class: 'editor__richtext tiptap',
          spellcheck: preferences.spellcheck ? 'true' : 'false',
        },
      },
    });
  }, [preferences.spellcheck, tiptap]);

  useEffect(() => {
    const close = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) setMenuOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setMenuOpen(false);
    };
    document.addEventListener('mousedown', close);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('mousedown', close);
      document.removeEventListener('keydown', escape);
    };
  }, []);

  if (!entry) {
    return <section className="editor editor--empty"><div><h2>Выберите запись</h2><p>Или создайте новую запись в средней колонке.</p></div></section>;
  }

  const createdAt = entry.createdAt ? new Date(entry.createdAt) : new Date(`${entry.date}T12:00:00`);
  const updatedAt = entry.updatedAt ? new Date(entry.updatedAt) : createdAt;
  const datePattern = preferences.timeFormat === '12h' ? 'd MMMM yyyy, hh:mm a' : 'd MMMM yyyy, HH:mm';
  const formatDateTime = (date: Date) => format(date, datePattern, { locale: ru });

  const toggleTag = (tagName: string) => {
    const tags = entry.tags.includes(tagName) ? entry.tags.filter((tag) => tag !== tagName) : [...entry.tags, tagName];
    dispatch(updateEntry({ id: entry.id, patch: { tags } }));
  };

  const copyText = async () => {
    const plainText = tiptap?.getText() ?? '';
    await navigator.clipboard.writeText(`${entry.title}\n\n${plainText}`);
    setMenuOpen(false);
  };

  const exportEntry = () => {
    const payload = JSON.stringify(entry, null, 2);
    const blob = new Blob([payload], { type: 'application/json;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `${(entry.title || 'entry').replace(/[\\/:*?"<>|]/g, '-')}.json`;
    link.click();
    URL.revokeObjectURL(url);
    setMenuOpen(false);
  };

  return (
    <section className="editor">
      <header className="editor__header">
        <div className="editor__meta">
          <div><span>Создано</span><strong>{formatDateTime(createdAt)}</strong></div>
          <div><span>Изменено</span><strong>{formatDateTime(updatedAt)}</strong></div>
        </div>

        <div className="editor__actions">
          <span className="saved-icon" title="Все изменения сохранены" aria-label="Все изменения сохранены"><Check /></span>
          <div className="editor__more" ref={menuRef}>
            <button type="button" title="Действия" aria-label="Действия" onClick={() => setMenuOpen((value) => !value)}>
              <MoreHorizontal />
            </button>
            {menuOpen && (
              <div className="editor__menu">
                <button type="button" onClick={() => { dispatch(duplicateEntry(entry.id)); setMenuOpen(false); }}><Copy />Дублировать запись</button>
                <button type="button" onClick={copyText}><Copy />Копировать текст</button>
                <button type="button" onClick={exportEntry}><Download />Экспортировать запись</button>
                <button type="button" onClick={() => { setInfoOpen(true); setMenuOpen(false); }}><Info />Информация о записи</button>
                <div className="editor__menu-separator" />
                <button type="button" className="editor__menu-danger" onClick={() => { setDeleteOpen(true); setMenuOpen(false); }}><Trash2 />Удалить запись</button>
              </div>
            )}
          </div>
          <ThemeToggle />
        </div>
      </header>

      <div className="editor__scroll">
        <article className="editor__document">
          <input className="editor__title" value={entry.title} onChange={(event) => dispatch(updateEntry({ id: entry.id, patch: { title: event.target.value } }))} placeholder="Название записи" />
          <div className="editor__tags">
            {availableTags.map((tag) => {
              const isActive = entry.tags.includes(tag.name);
              return <button key={tag.id} type="button" className={isActive ? 'active' : ''} style={isActive ? { borderColor: tag.color, boxShadow: `inset 0 0 0 1px ${tag.color}` } : undefined} onClick={() => toggleTag(tag.name)}><span className="editor__tag-dot" style={{ background: tag.color }} />{tag.name}</button>;
            })}
          </div>
          <EditorContent editor={tiptap} />
        </article>
      </div>

      {preferences.showToolbar && <EditorToolbar editor={tiptap} />}

      <Modal open={infoOpen} title="Информация о записи" onClose={() => setInfoOpen(false)} footer={<button type="button" className="modal__button" onClick={() => setInfoOpen(false)}>Закрыть</button>}>
        <div className="editor__info"><p><span>Создано</span><strong>{formatDateTime(createdAt)}</strong></p><p><span>Последнее изменение</span><strong>{formatDateTime(updatedAt)}</strong></p><p><span>Символов</span><strong>{tiptap?.getText().length ?? 0}</strong></p><p><span>Тегов</span><strong>{entry.tags.length}</strong></p></div>
      </Modal>

      <Modal open={deleteOpen} title="Удалить запись?" danger onClose={() => setDeleteOpen(false)} footer={<><button type="button" className="modal__button" onClick={() => setDeleteOpen(false)}>Отмена</button><button type="button" className="modal__button modal__button--danger" onClick={() => { dispatch(deleteEntry(entry.id)); setDeleteOpen(false); }}>Удалить</button></>}>
        <p>Запись «{entry.title || 'Без названия'}» будет удалена без возможности восстановления.</p>
      </Modal>
    </section>
  );
}
