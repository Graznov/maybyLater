import { Moon, Sun } from 'lucide-react';
import { useAppDispatch, useAppSelector } from '../../app/hooks';
import { setPreference } from '../../features/preferences/preferencesSlice';
import { useI18n } from '../../i18n/useI18n';
import './ThemeToggle.css';

export function ThemeToggle() {
  const dispatch = useAppDispatch();
  const preference = useAppSelector((state) => state.preferences.theme);
  const { t } = useI18n();
  const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  const effectiveDark = preference === 'dark' || (preference === 'system' && systemDark);

  const toggle = () => {
    dispatch(setPreference({ key: 'theme', value: effectiveDark ? 'light' : 'dark' }));
  };

  return (
    <button className="theme-toggle" onClick={toggle} title={t('changeTheme')} aria-label={t('changeTheme')}>
      {effectiveDark ? <Sun size={18} /> : <Moon size={18} />}
    </button>
  );
}
