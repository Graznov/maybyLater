import { Plus, Search } from 'lucide-react';
import { format } from 'date-fns';
import { useAppDispatch, useAppSelector } from '../../app/hooks';
import { addEntry, selectEntry, setQuery } from '../../features/entries/entriesSlice';
import './Entries.css';
import { EntryThumbnail } from './EntryThumbnail';

export function Entries({ all = false }: { all?: boolean }) {
  const dispatch = useAppDispatch();
  const state = useAppSelector((x) => x.entries);
  const language = useAppSelector((x) => x.preferences.language);
  const en = language === 'en';
  const items = state.items
    .filter((entry) => (all || entry.date === state.selectedDate) && (!state.tag || entry.tags.includes(state.tag)) && (`${entry.title} ${entry.content}`.toLowerCase().includes(state.query.toLowerCase())))
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));

  return (
    <section className="entries">
      <div className="entries__tools">
        <div className="search"><Search /><input value={state.query} onChange={(e) => dispatch(setQuery(e.target.value))} placeholder={en ? 'Search' : 'Поиск'} /></div>
        <button className="entries__add" onClick={() => dispatch(addEntry())}><Plus /> {en ? 'New' : 'Новая'}</button>
      </div>
      <div className="entries__list">
        {items.length ? items.map((entry) => (
          <button key={entry.id} onClick={() => dispatch(selectEntry(entry.id))} className={`entry-card ${entry.id === state.selectedId ? 'active' : ''}`}>
            <div className="entry-card__layout">
              <div className="entry-card__body">
                <div className="entry-card__heading"><b>{entry.title || (en ? 'Untitled' : 'Без названия')}</b><time>{format(new Date(entry.updatedAt), 'HH:mm')}</time></div>
                <p>{entry.content.replace(/<[^>]*>/g, ' ') || (en ? 'Empty entry' : 'Пустая запись')}</p>
                {entry.tags.length > 0 && <footer>{entry.tags.map((tag) => <span key={tag}>{tag}</span>)}</footer>}
              </div>
              <EntryThumbnail content={entry.content} />
            </div>
          </button>
        )) : <div className="entries__empty">{en ? <>No entries yet.<br />Create your first one ✨</> : <>Записей пока нет.<br />Создай первую ✨</>}</div>}
      </div>
    </section>
  );
}
