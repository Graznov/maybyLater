
import { useMemo, useState } from 'react';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import {
  addMonths,
  eachDayOfInterval,
  endOfMonth,
  endOfWeek,
  format,
  isSameMonth,
  startOfMonth,
  startOfWeek,
  subMonths,
} from 'date-fns';
import { ru } from 'date-fns/locale';
import { useAppDispatch, useAppSelector } from '../../app/hooks';
import { selectDate } from '../../features/entries/entriesSlice';
import './Calendar.css';

const WEEKDAYS = ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Вс'];

export function Calendar() {
  const dispatch = useAppDispatch();
  const selectedDate = useAppSelector((state) => state.entries.selectedDate);
  const entries = useAppSelector((state) => state.entries.items);
  const dateColors = useAppSelector((state) => state.calendar.dateColors);

  const [month, setMonth] = useState(
    () => new Date(`${selectedDate}T12:00:00`),
  );

  const days = useMemo(
    () =>
      eachDayOfInterval({
        start: startOfWeek(startOfMonth(month), { weekStartsOn: 1 }),
        end: endOfWeek(endOfMonth(month), { weekStartsOn: 1 }),
      }),
    [month],
  );

  const entryDates = useMemo(
    () => new Set(entries.map((entry) => entry.date)),
    [entries],
  );

  const goToToday = () => {
    const today = new Date();
    setMonth(today);
    dispatch(selectDate(format(today, 'yyyy-MM-dd')));
  };

  return (
    <div className="calendar">
      <div className="calendar__head">
        <b>{format(month, 'LLLL yyyy', { locale: ru })}</b>

        <div className="calendar__controls">
          <button
            className="calendar__today"
            type="button"
            onClick={goToToday}
            aria-label="Перейти к сегодняшней дате"
            title="Перейти к сегодняшней дате"
          >
            Сегодня
          </button>

          <button
            className="calendar__nav"
            type="button"
            onClick={() => setMonth(subMonths(month, 1))}
            aria-label="Предыдущий месяц"
          >
            <ChevronLeft />
          </button>

          <button
            className="calendar__nav"
            type="button"
            onClick={() => setMonth(addMonths(month, 1))}
            aria-label="Следующий месяц"
          >
            <ChevronRight />
          </button>
        </div>
      </div>

      <div className="calendar__week">
        {WEEKDAYS.map((weekday) => (
          <span key={weekday}>{weekday}</span>
        ))}
      </div>

      <div className="calendar__grid">
        {days.map((day) => {
          const key = format(day, 'yyyy-MM-dd');
          const color = dateColors[key];
          const isSelected = key === selectedDate;

          return (
            <button
              type="button"
              key={key}
              className={[
                isSelected ? 'selected' : '',
                !isSameMonth(day, month) ? 'muted' : '',
                color ? 'has-color' : '',
              ]
                .filter(Boolean)
                .join(' ')}
              style={
                color
                  ? ({ '--calendar-date-color': color } as React.CSSProperties)
                  : undefined
              }
              onClick={() => dispatch(selectDate(key))}
              title={format(day, 'd MMMM yyyy', { locale: ru })}
            >
              <span>{format(day, 'd')}</span>
              {entryDates.has(key) && <i aria-hidden="true" />}
            </button>
          );
        })}
      </div>
    </div>
  );
}
