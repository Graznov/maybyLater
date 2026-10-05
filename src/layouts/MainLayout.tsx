import { Outlet } from 'react-router-dom'; import { Sidebar } from '../components/Sidebar/Sidebar'; import { ThemeToggle } from '../components/ThemeToggle/ThemeToggle'; import './MainLayout.css';
export function MainLayout(){return <div className="main-layout"><Sidebar/><ThemeToggle/><Outlet/></div>}
