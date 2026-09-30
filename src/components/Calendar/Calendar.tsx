import { ChevronLeft, ChevronRight } from 'lucide-react';
import { addMonths, eachDayOfInterval, endOfMonth, endOfWeek, format, isSameDay, isSameMonth, parseISO, startOfMonth, startOfWeek, subMonths } from 'date-fns';
import { ru } from 'date-fns/locale';
import { useState } from 'react';
import { useAppDispatch,useAppSelector } from '../../app/hooks';
import { selectDate } from '../../store/entriesSlice';
import './Calendar.css';

export function Calendar(){
    const dispatch=useAppDispatch();
    const {items,selectedDate}=useAppSelector(s=>s.entries);
    const [month,setMonth]=useState(parseISO(selectedDate));
    const start=startOfWeek(startOfMonth(month),{weekStartsOn:1}),
        end=endOfWeek(endOfMonth(month),{weekStartsOn:1});
    const days=eachDayOfInterval({start,end});
    const marked=new Set(items.map(e=>e.date));
    return <
        section className="calendar">
        <header className="calendar__header">
            <strong>{format(month,'LLLL yyyy',{locale:ru})}
            </strong>
            <div>
                <button
                    onClick={()=>setMonth(subMonths(month,1))}>
                    <ChevronLeft size={17}/>
                </button>
                <button
                    onClick={()=>setMonth(addMonths(month,1))}>
                    <ChevronRight size={17}/>
                </button></div>
        </header>
        <div
            className="calendar__weekdays">{['Пн','Вт','Ср','Чт','Пт','Сб','Вс'].map(x=><span key={x}>{x}</span>)}
        </div>
        <div className="calendar__grid">{days.map(day=>{const iso=format(day,'yyyy-MM-dd');
            const selected=isSameDay(day,parseISO(selectedDate));
            return <button key={iso} className={`${!isSameMonth(day,month)?'muted ':''}${selected?'selected':''}`}
                           onClick={()=>dispatch(selectDate(iso))}>
                <span>{format(day,'d')}</span>
                {marked.has(iso)&&<i/>}</button>})}
        </div>
    </section>}
