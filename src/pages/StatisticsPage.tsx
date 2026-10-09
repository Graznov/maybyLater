import { useMemo } from 'react';
import {
  BarChart3,
  BookOpenText,
  CalendarDays,
  Flame,
  Hash,
  Trophy,
} from 'lucide-react';
import { eachDayOfInterval, endOfYear, format, getDay, startOfYear } from 'date-fns';
import { ru } from 'date-fns/locale';
import { useAppSelector } from '../app/hooks';
import './StatisticsPage.css';

const MONTHS = [
  'Янв', 'Фев', 'Мар', 'Апр', 'Май', 'Июн',
  'Июл', 'Авг', 'Сен', 'Окт', 'Ноя', 'Дек',
];

function plainText(value: string) {
  return value
    .replace(/<[^>]*>/g, ' ')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/\s+/g, ' ')
    .trim();
}

function wordCount(value: string) {
  const text = plainText(value);
  return text ? text.split(/\s+/).length : 0;
}

function longestStreak(dateKeys: string[]) {
  const days = [...new Set(dateKeys)].sort();
  let best = 0;
  let current = 0;
  let previous: number | null = null;

  for (const key of days) {
    const timestamp = new Date(`${key}T12:00:00`).getTime();
    const delta = previous === null ? null : Math.round((timestamp - previous) / 86_400_000);
    current = delta === 1 ? current + 1 : 1;
    best = Math.max(best, current);
    previous = timestamp;
  }

  return best;
}

function currentStreak(dateKeys: string[]) {
  const set = new Set(dateKeys);
  const cursor = new Date();
  let streak = 0;

  // A streak may still be alive when today's entry has not been written yet.
  if (!set.has(format(cursor, 'yyyy-MM-dd'))) {
    cursor.setDate(cursor.getDate() - 1);
  }

  while (set.has(format(cursor, 'yyyy-MM-dd'))) {
    streak += 1;
    cursor.setDate(cursor.getDate() - 1);
  }

  return streak;
}

export function StatisticsPage() {
  const entries = useAppSelector((state) => state.entries.items);
  const tags = useAppSelector((state) => state.tags.items);
  const year = new Date().getFullYear();

  const stats = useMemo(() => {
    const dateCounts = new Map<string, number>();
    const monthCounts = Array.from({ length: 12 }, () => 0);
    const tagCounts = new Map<string, number>();
    let totalWords = 0;

    entries.forEach((entry) => {
      dateCounts.set(entry.date, (dateCounts.get(entry.date) ?? 0) + 1);
      totalWords += wordCount(entry.content);

      const entryDate = new Date(`${entry.date}T12:00:00`);
      if (entryDate.getFullYear() === year) {
        monthCounts[entryDate.getMonth()] += 1;
      }

      entry.tags.forEach((tag) => {
        tagCounts.set(tag, (tagCounts.get(tag) ?? 0) + 1);
      });
    });

    const dates = [...dateCounts.keys()];
    const sortedEntries = [...entries].sort((a, b) => a.date.localeCompare(b.date));
    const weekdayCounts = Array.from({ length: 7 }, () => 0);

    entries.forEach((entry) => {
      const day = getDay(new Date(`${entry.date}T12:00:00`));
      weekdayCounts[day] += 1;
    });

    const weekdayNames = ['воскресенье', 'понедельник', 'вторник', 'среда', 'четверг', 'пятница', 'суббота'];
    const bestWeekdayIndex = weekdayCounts.indexOf(Math.max(...weekdayCounts));
    const maxMonth = Math.max(...monthCounts, 1);
    const activeMonthIndex = monthCounts.indexOf(Math.max(...monthCounts));

    const popularTags = [...tagCounts.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, 6)
      .map(([name, count]) => ({
        name,
        count,
        color: tags.find((tag) => tag.name === name)?.color ?? '#7b8ba3',
      }));

    return {
      dateCounts,
      monthCounts,
      maxMonth,
      popularTags,
      totalWords,
      currentStreak: currentStreak(dates),
      longestStreak: longestStreak(dates),
      firstDate: sortedEntries[0]?.date ?? null,
      averageWords: entries.length ? Math.round(totalWords / entries.length) : 0,
      bestWeekday: entries.length ? weekdayNames[bestWeekdayIndex] : '—',
      activeMonth: Math.max(...monthCounts) > 0 ? MONTHS[activeMonthIndex] : '—',
    };
  }, [entries, tags, year]);

  const yearDays = useMemo(
    () => eachDayOfInterval({ start: startOfYear(new Date(year, 0, 1)), end: endOfYear(new Date(year, 0, 1)) }),
    [year],
  );

  const firstDayOffset = (getDay(yearDays[0]) + 6) % 7;
  const maxDaily = Math.max(...stats.dateCounts.values(), 1);

  return (
    <main className="statistics-page">
      <header className="statistics-header">
        <div>
          <span className="statistics-eyebrow">Обзор активности</span>
          <h1>Статистика</h1>
          <p>Небольшой взгляд на то, как складывается история вашего дневника.</p>
        </div>
        <div className="statistics-year">{year}</div>
      </header>

      <div className="statistics-scroll page-scroll">
      <section className="stat-cards">
        <article className="stat-card">
          <span className="stat-icon"><BookOpenText size={20} /></span>
          <div><strong>{entries.length}</strong><span>Всего записей</span></div>
        </article>
        <article className="stat-card">
          <span className="stat-icon"><Flame size={20} /></span>
          <div><strong>{stats.currentStreak}</strong><span>Текущая серия, дней</span></div>
        </article>
        <article className="stat-card">
          <span className="stat-icon"><Trophy size={20} /></span>
          <div><strong>{stats.longestStreak}</strong><span>Лучшая серия, дней</span></div>
        </article>
        <article className="stat-card">
          <span className="stat-icon"><Hash size={20} /></span>
          <div><strong>{stats.totalWords.toLocaleString('ru-RU')}</strong><span>Написано слов</span></div>
        </article>
      </section>

      <section className="statistics-panel activity-panel">
        <div className="panel-heading">
          <div><h2>Активность за год</h2><p>Каждая клетка — один день. Насыщенность зависит от количества записей.</p></div>
          <div className="activity-legend"><span>Меньше</span><i data-level="0" /><i data-level="1" /><i data-level="2" /><i data-level="3" /><i data-level="4" /><span>Больше</span></div>
        </div>
        <div className="heatmap-scroll">
          <div className="heatmap" style={{ '--offset': firstDayOffset } as React.CSSProperties}>
            {Array.from({ length: firstDayOffset }).map((_, index) => <span className="heatmap-spacer" key={`spacer-${index}`} />)}
            {yearDays.map((day) => {
              const key = format(day, 'yyyy-MM-dd');
              const count = stats.dateCounts.get(key) ?? 0;
              const level = count === 0 ? 0 : Math.max(1, Math.ceil((count / maxDaily) * 4));
              return <span key={key} className="heatmap-day" data-level={level} title={`${format(day, 'd MMMM', { locale: ru })}: ${count} записей`} />;
            })}
          </div>
        </div>
      </section>

      <div className="statistics-grid">
        <section className="statistics-panel">
          <div className="panel-heading"><div><h2>Записи по месяцам</h2><p>Активность в {year} году</p></div><BarChart3 size={21} /></div>
          <div className="month-chart">
            {stats.monthCounts.map((count, index) => (
              <div className="month-column" key={MONTHS[index]}>
                <span className="month-value">{count || ''}</span>
                <div className="month-track"><div className="month-fill" style={{ height: `${Math.max(count ? 12 : 3, (count / stats.maxMonth) * 100)}%` }} /></div>
                <span>{MONTHS[index]}</span>
              </div>
            ))}
          </div>
        </section>

        <section className="statistics-panel">
          <div className="panel-heading"><div><h2>Популярные теги</h2><p>Какие темы встречаются чаще всего</p></div><Hash size={21} /></div>
          <div className="tag-stats">
            {stats.popularTags.length ? stats.popularTags.map((tag) => {
              const max = stats.popularTags[0]?.count ?? 1;
              return (
                <div className="tag-stat" key={tag.name}>
                  <div className="tag-stat-row"><span><i style={{ background: tag.color }} />{tag.name}</span><strong>{tag.count}</strong></div>
                  <div className="tag-stat-track"><div style={{ width: `${(tag.count / max) * 100}%`, background: tag.color }} /></div>
                </div>
              );
            }) : <div className="empty-stat">Добавьте теги к записям — здесь появится статистика.</div>}
          </div>
        </section>
      </div>

      <section className="statistics-panel numbers-panel">
        <div className="panel-heading"><div><h2>Дневник в цифрах</h2><p>Несколько деталей о ваших записях</p></div><CalendarDays size={21} /></div>
        <div className="number-grid">
          <div><span>Средняя длина записи</span><strong>{stats.averageWords} слов</strong></div>
          <div><span>Самый активный день недели</span><strong>{stats.bestWeekday}</strong></div>
          <div><span>Самый активный месяц</span><strong>{stats.activeMonth}</strong></div>
          <div><span>Первая запись</span><strong>{stats.firstDate ? format(new Date(`${stats.firstDate}T12:00:00`), 'd MMMM yyyy', { locale: ru }) : '—'}</strong></div>
        </div>
      </section>
      </div>
    </main>
  );
}
