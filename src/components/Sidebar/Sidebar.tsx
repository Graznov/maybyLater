import { BarChart3, BookOpen, CalendarDays, NotebookPen, Settings, Tags } from 'lucide-react';
import { NavLink } from 'react-router-dom';
import './Sidebar.css';


const navigation=[['Сегодня','/',BookOpen],['Календарь','/calendar',CalendarDays],['Все записи','/entries',NotebookPen],['Теги','/tags',Tags],['Статистика','/statistics',BarChart3]] as const;
const tags=[['Личное','#5a8dee'],['Работа','#54b689'],['Идеи','#a477d4']];

export function Sidebar(){
    return <aside className="sidebar">
                <div className="sidebar__logo">
                    <div className="sidebar__logo-icon">
                        <NotebookPen size={21}/>
                    </div>
                    <span className="sidebar__logo-text">Мой дневник</span>
                </div>
        <nav className="sidebar__navigation">{navigation.map(([title,path,Icon])=><NavLink key={path} to={path} className={({isActive})=>`sidebar__nav-item ${isActive?'sidebar__nav-item--active':''}`}><Icon size={19}/><span>{title}</span></NavLink>)}</nav><div className="sidebar__tags"><div className="sidebar__section-title">Теги</div>{tags.map(([title,color])=><button className="sidebar__tag" key={title}><span className="sidebar__tag-dot" style={{backgroundColor:color}}/><span>{title}</span></button>)}</div><div className="sidebar__bottom"><NavLink to="/settings" className={({isActive})=>`sidebar__nav-item ${isActive?'sidebar__nav-item--active':''}`}><Settings size={19}/><span>Настройки</span></NavLink></div></aside>}
