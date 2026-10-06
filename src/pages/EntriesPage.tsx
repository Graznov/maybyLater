import { useAppSelector } from '../app/hooks';
import { Entries } from '../components/Entries/Entries';
import { Editor } from '../components/Editor/Editor';
import './pages.css';

export function EntriesPage() {
  const en = useAppSelector((state) => state.preferences.language) === 'en';
  return <main className="journal-grid"><div className="journal-middle"><div className="section-heading"><h2>{en ? 'All entries' : 'Все записи'}</h2><p>{en ? 'Search across your journal' : 'Поиск по всему дневнику'}</p></div><Entries all /></div><Editor /></main>;
}
