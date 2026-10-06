import {
  ChevronLeft,
  ChevronRight,
  Eraser,
  ExternalLink,
  Palette,
} from 'lucide-react';
import {
  eachDayOfInterval,
  endOfMonth,
  format,
  getDay,
  isToday,
  startOfMonth,
} from 'date-fns';
import { ru } from 'date-fns/locale';
import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAppDispatch, useAppSelector } from '../app/hooks';
import {
  clearDateColor,
  setDateColor,
} from '../features/calendar/calendarSlice';
import { selectDate } from '../features/entries/entriesSlice';
import './YearCalendar.css';

const WEEKDAYS = ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Вс'];

const DATE_COLORS = [
  '#ef6b6b',
  '#f59e42',
  '#f2c94c',
  '#65b96e',
  '#4aa3df',
  '#5f7eea',
  '#9b72df',
  '#e36ca8',
] as const;

function dateKey(date: Date) {
  return format(date, 'yyyy-MM-dd');
}

function MonthCard({
  month,
  selectedDate,
  entryDates,
  dateColors,
  onSelect,
}: {
  month: Date;
  selectedDate: string;
  entryDates: Set<string>;
  dateColors: Record<string, string>;
  onSelect: (date: string) => void;
}) {
  const days = eachDayOfInterval({
    start: startOfMonth(month),
    end: endOfMonth(month),
  });

  const mondayOffset = (getDay(startOfMonth(month)) + 6) % 7;

  return (
    <section className="year-month">
      <h2>{format(month, 'LLLL', { locale: ru })}</h2>

      <div className="year-month__weekdays">
        {WEEKDAYS.map((weekday) => (
          <span key={weekday}>{weekday}</span>
        ))}
      </div>

      <div className="year-month__days">
        {Array.from({ length: mondayOffset }).map((_, index) => (
          <span className="year-day year-day--empty" key={`empty-${index}`} />
        ))}

        {days.map((day) => {
          const key = dateKey(day);
          const color = dateColors[key];
          const hasEntries = entryDates.has(key);

          return (
            <button
              type="button"
              key={key}
              className={[
                'year-day',
                selectedDate === key ? 'year-day--selected' : '',
                isToday(day) ? 'year-day--today' : '',
              ]
                .filter(Boolean)
                .join(' ')}
              style={
                color
                  ? ({ '--date-color': color } as React.CSSProperties)
                  : undefined
              }
              onClick={() => onSelect(key)}
              title={format(day, 'd MMMM yyyy', { locale: ru })}
            >
              <span>{format(day, 'd')}</span>
              {hasEntries && <i className="year-day__entry-dot" />}
            </button>
          );
        })}
      </div>
    </section>
  );
}

export function CalendarPage() {
  const dispatch = useAppDispatch();
  const navigate = useNavigate();
  const selectedDate = useAppSelector((state) => state.entries.selectedDate);
  const entries = useAppSelector((state) => state.entries.items);
  const dateColors = useAppSelector((state) => state.calendar.dateColors);

  const [year, setYear] = useState(() =>
    Number(selectedDate.slice(0, 4)) || new Date().getFullYear(),
  );

  const months = useMemo(
    () => Array.from({ length: 12 }, (_, month) => new Date(year, month, 1)),
    [year],
  );

  const entryDates = useMemo(
    () => new Set(entries.map((entry) => entry.date)),
    [entries],
  );

  const selectedLabel = format(
    new Date(`${selectedDate}T12:00:00`),
    'd MMMM yyyy',
    { locale: ru },
  );

  const handleSelectDate = (date: string) => {
    dispatch(selectDate(date));
  };

  const handleOpenDay = () => {
    navigate('/');
  };

  return (
    <main className="year-calendar-page">
      <header className="year-calendar-toolbar">
        <div className="year-calendar-toolbar__year">
          <button
            type="button"
            className="icon-button"
            onClick={() => setYear((current) => current - 1)}
            aria-label="Предыдущий год"
          >
            <ChevronLeft size={20} />
          </button>

          <div>
            <span>Календарь</span>
            <h1>{year}</h1>
          </div>

          <button
            type="button"
            className="icon-button"
            onClick={() => setYear((current) => current + 1)}
            aria-label="Следующий год"
          >
            <ChevronRight size={20} />
          </button>
        </div>

        <div className="year-calendar-toolbar__actions">
          <div className="selected-date-info">
            <span>Выбрано</span>
            <strong>{selectedLabel}</strong>
          </div>

          <div className="date-color-picker">
            <span className="date-color-picker__label">
              <Palette size={17} />
              Цвет даты
            </span>

            <div className="date-color-picker__colors">
              {DATE_COLORS.map((color) => (
                <button
                  type="button"
                  key={color}
                  className={
                    dateColors[selectedDate] === color
                      ? 'date-color-swatch selected'
                      : 'date-color-swatch'
                  }
                  style={{ background: color }}
                  onClick={() =>
                    dispatch(setDateColor({ date: selectedDate, color }))
                  }
                  aria-label={`Установить цвет ${color}`}
                />
              ))}

              <button
                type="button"
                className="date-color-clear"
                onClick={() => dispatch(clearDateColor(selectedDate))}
                disabled={!dateColors[selectedDate]}
                title="Убрать цвет"
              >
                <Eraser size={17} />
              </button>
            </div>
          </div>

          <button
            type="button"
            className="open-day-button"
            onClick={handleOpenDay}
          >
            Открыть день
            <ExternalLink size={17} />
          </button>
        </div>
      </header>

      <div className="year-calendar-scroll">
        <div className="year-calendar-grid">
          {months.map((month) => (
            <MonthCard
              key={month.getMonth()}
              month={month}
              selectedDate={selectedDate}
              entryDates={entryDates}
              dateColors={dateColors}
              onSelect={handleSelectDate}
            />
          ))}
        </div>
      </div>
    </main>
  );
}
