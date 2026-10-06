import { useEffect } from 'react';
import { Outlet } from 'react-router-dom';
import { useAppSelector } from '../app/hooks';
import { Sidebar } from '../components/Sidebar/Sidebar';
import { ThemeToggle } from '../components/ThemeToggle/ThemeToggle';
import './MainLayout.css';

export function MainLayout() {
  const preferences = useAppSelector((state) => state.preferences);

  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const apply = () => {
      const theme = preferences.theme === 'system'
        ? (media.matches ? 'dark' : 'light')
        : preferences.theme;
      document.documentElement.dataset.theme = theme;
      document.documentElement.lang = preferences.language;
      document.documentElement.dataset.editorFont = preferences.editorFontSize;
      document.documentElement.dataset.editorWidth = preferences.editorWidth;
      document.documentElement.dataset.sidebarBackground = String(preferences.sidebarBackground);
    };
    apply();
    media.addEventListener('change', apply);
    return () => media.removeEventListener('change', apply);
  }, [preferences]);

  return (
    <div className="main-layout">
      <Sidebar />
      <ThemeToggle />
      <Outlet />
    </div>
  );
}
