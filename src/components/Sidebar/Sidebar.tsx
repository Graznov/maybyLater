import { FormEvent, useEffect, useState } from 'react';
import { BarChart3, BookOpen, CalendarDays, NotebookPen, Pencil, Settings, SquarePen, Tags } from 'lucide-react';
import { NavLink } from 'react-router-dom';
import { useAppDispatch, useAppSelector } from '../../app/hooks';
import { setTag } from '../../features/entries/entriesSlice';
import { setJournalName } from '../../features/preferences/preferencesSlice';
import { useI18n } from '../../i18n/useI18n';
import { quotes } from '../../data/quotes';
import { Modal } from '../Modal/Modal';
import './Sidebar.css';

export function Sidebar() {
  const dispatch = useAppDispatch();
  const { t } = useI18n();
  const activeTag = useAppSelector((state) => state.entries.tag);
  const tags = useAppSelector((state) => state.tags.items);
  const journalName = useAppSelector((state) => state.preferences.journalName);
  const [isNameModalOpen, setIsNameModalOpen] = useState(false);
  const [nameDraft, setNameDraft] = useState(journalName);
  const [quote] = useState(() => quotes[Math.floor(Math.random() * quotes.length)] ?? '');

  const navigation = [
    [t('today'), '/', SquarePen],
    [t('calendar'), '/calendar', CalendarDays],
    [t('allEntries'), '/entries', NotebookPen],
    [t('tags'), '/tags', Tags],
    [t('statistics'), '/statistics', BarChart3],
  ] as const;

  useEffect(() => {
    if (isNameModalOpen) setNameDraft(journalName);
  }, [isNameModalOpen, journalName]);

  const handleSaveName = (event: FormEvent) => {
    event.preventDefault();
    if (!nameDraft.trim()) return;
    dispatch(setJournalName(nameDraft));
    setIsNameModalOpen(false);
  };

  return (
    <>
      <aside className="sidebar">
        <button type="button" className="sidebar__logo" onClick={() => setIsNameModalOpen(true)} title={t('editName')}>
          <BookOpen size={30} strokeWidth={2} aria-hidden="true" />
          <b>{journalName}</b>
          <Pencil className="sidebar__logo-edit" size={14} />
        </button>

        <nav>
          {navigation.map(([title, path, Icon]) => (
            <NavLink key={path} to={path} className={({ isActive }) => `sidebar__item ${isActive ? 'active' : ''}`}>
              <Icon size={20} />
              <span>{title}</span>
            </NavLink>
          ))}
        </nav>

        <div className="sidebar__divider" />
        <div className="sidebar__tags">
          <strong>{t('tags')}</strong>
          <div className="sidebar__tags-list">
          {tags.map((tag) => (
            <button type="button" key={tag.id} className={activeTag === tag.name ? 'selected' : ''} onClick={() => dispatch(setTag(activeTag === tag.name ? null : tag.name))}>
              <i style={{ background: tag.color }} />
              <span>{tag.name}</span>
            </button>
          ))}
          </div>
          <NavLink to="/tags" className="sidebar__new-tag"><span>＋</span>{t('newTag')}</NavLink>
        </div>

        <div className="sidebar__quote" title={quote}>
          {quote}
        </div>
        <div className="sidebar__bottom">
          <NavLink to="/settings" className="sidebar__item"><Settings size={20} /><span>{t('settings')}</span></NavLink>
        </div>
      </aside>

      <Modal open={isNameModalOpen} title={t('journalName')} onClose={() => setIsNameModalOpen(false)} footer={
        <>
          <button type="button" className="modal__button" onClick={() => setIsNameModalOpen(false)}>{t('cancel')}</button>
          <button type="submit" form="journal-name-form" className="modal__button modal__button--primary" disabled={!nameDraft.trim()}>{t('save')}</button>
        </>
      }>
        <form id="journal-name-form" onSubmit={handleSaveName}>
          <label htmlFor="journal-name">{t('nameQuestion')}</label>
          <input id="journal-name" className="modal__input" value={nameDraft} onChange={(event) => setNameDraft(event.target.value)} maxLength={40} autoFocus placeholder={t('nameExample')} />
        </form>
      </Modal>
    </>
  );
}
