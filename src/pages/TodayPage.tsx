import { Calendar } from '../components/Calendar/Calendar';
import { Editor } from '../components/Editor/Editor';
import { Entries } from '../components/Entries/Entries';
import './pages.css';

export function TodayPage() {
  return (
    <main className="journal-grid">
      <div className="journal-middle">
        <Calendar />
        <Entries />
      </div>

      <Editor />
    </main>
  );
}
