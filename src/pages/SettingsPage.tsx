import { ChangeEvent, useRef, useState } from 'react';
import { Database, Download, Languages, Monitor, Palette, RotateCcw, Settings2, Trash2, Upload } from 'lucide-react';
import { useAppDispatch, useAppSelector } from '../app/hooks';
import { setJournalName, setPreference, type PreferencesState } from '../features/preferences/preferencesSlice';
import { Modal } from '../components/Modal/Modal';
import './SettingsPage.css';

const APP_KEYS = ['maybeLater.entries.v1', 'maybeLater.tags.v1', 'maybeLater.calendarColors.v1', 'maybeLater.preferences.v1'];

export function SettingsPage() {
  const dispatch = useAppDispatch();
  const p = useAppSelector((state) => state.preferences);
  const english = p.language === 'en';
  const fileRef = useRef<HTMLInputElement>(null);
  const [confirmReset, setConfirmReset] = useState(false);

  const set = <K extends keyof PreferencesState>(key: K, value: PreferencesState[K]) => {
    dispatch(setPreference({ key, value }));
  };

  const exportData = () => {
    const data = Object.fromEntries(APP_KEYS.map((key) => [key, localStorage.getItem(key)]));
    const blob = new Blob([JSON.stringify({ app: 'Maybe Later Journal', version: 1, data }, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `maybe-later-backup-${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const importData = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    try {
      const backup = JSON.parse(await file.text());
      APP_KEYS.forEach((key) => {
        const value = backup?.data?.[key];
        if (typeof value === 'string') localStorage.setItem(key, value);
      });
      window.location.reload();
    } catch {
      event.target.value = '';
    }
  };

  const resetAll = () => {
    APP_KEYS.forEach((key) => localStorage.removeItem(key));
    window.location.reload();
  };

  return (
    <main className="settings-page">
      <header className="settings-header">
        <span>{english ? 'APPLICATION' : 'ПРИЛОЖЕНИЕ'}</span>
        <h1>{english ? 'Settings' : 'Настройки'}</h1>
        <p>{english ? 'Make the journal comfortable for the way you write.' : 'Настройте дневник так, чтобы им было удобно пользоваться именно вам.'}</p>
      </header>

      <div className="settings-scroll page-scroll">
      <div className="settings-grid">
        <SettingsSection icon={<Palette />} title={english ? 'Appearance' : 'Внешний вид'} description={english ? 'Theme and workspace appearance.' : 'Тема и внешний вид рабочего пространства.'}>
          <SettingRow label={english ? 'Theme' : 'Тема'}>
            <Segmented value={p.theme} onChange={(v) => set('theme', v as PreferencesState['theme'])} options={english ? [['system','System'],['light','Light'],['dark','Dark']] : [['system','Системная'],['light','Светлая'],['dark','Тёмная']]} />
          </SettingRow>
          <SettingRow label={english ? 'Sidebar mountain background' : 'Горный фон Sidebar'}><Switch checked={p.sidebarBackground} onChange={(v) => set('sidebarBackground', v)} /></SettingRow>
          <SettingRow label={english ? 'Translucent window (experimental)' : 'Полупрозрачное окно (экспериментально)'}><Switch checked={p.windowTransparency} onChange={(v) => set('windowTransparency', v)} /></SettingRow>
          <p className="transparency-hint">{english ? 'Restart the desktop app if the effect is not visible. Disable to restore the original appearance.' : 'Если эффект не виден, перезапустите приложение. Выключите переключатель, чтобы вернуть прежний вид.'}</p>
        </SettingsSection>

        <SettingsSection icon={<Languages />} title={english ? 'Language' : 'Язык'} description={english ? 'Interface language. Your entries are never translated.' : 'Язык интерфейса. Текст ваших записей не переводится.'}>
          <SettingRow label={english ? 'Interface language' : 'Язык интерфейса'}>
            <Segmented value={p.language} onChange={(v) => set('language', v as 'ru' | 'en')} options={[['ru','Русский'],['en','English']]} />
          </SettingRow>
        </SettingsSection>

        <SettingsSection icon={<Settings2 />} title={english ? 'Journal' : 'Дневник'} description={english ? 'General journal preferences.' : 'Основные параметры дневника.'}>
          <SettingRow label={english ? 'Journal name' : 'Название дневника'}><input className="settings-input" value={p.journalName} onChange={(e) => dispatch(setJournalName(e.target.value || ' '))} onBlur={(e) => !e.target.value.trim() && dispatch(setJournalName(english ? 'My journal' : 'Мой дневник'))} /></SettingRow>
          <SettingRow label={english ? 'First day of week' : 'Первый день недели'}><Segmented value={p.firstDayOfWeek} onChange={(v) => set('firstDayOfWeek', v as 'monday' | 'sunday')} options={english ? [['monday','Monday'],['sunday','Sunday']] : [['monday','Понедельник'],['sunday','Воскресенье']]} /></SettingRow>
          <SettingRow label={english ? 'Time format' : 'Формат времени'}><Segmented value={p.timeFormat} onChange={(v) => set('timeFormat', v as '24h' | '12h')} options={[['24h','24h'],['12h','12h']]} /></SettingRow>
          <SettingRow label={english ? 'Show word count' : 'Показывать количество слов'}><Switch checked={p.showWordCount} onChange={(v) => set('showWordCount', v)} /></SettingRow>
          <SettingRow label={english ? 'Show entry time' : 'Показывать время записи'}><Switch checked={p.showEntryTime} onChange={(v) => set('showEntryTime', v)} /></SettingRow>
        </SettingsSection>

        <SettingsSection icon={<Monitor />} title={english ? 'Editor' : 'Редактор'} description={english ? 'Reading, writing and autosave preferences.' : 'Параметры текста, набора и автосохранения.'}>
          <SettingRow label={english ? 'Text size' : 'Размер текста'}><Segmented value={p.editorFontSize} onChange={(v) => set('editorFontSize', v as PreferencesState['editorFontSize'])} options={english ? [['small','Small'],['medium','Medium'],['large','Large']] : [['small','Малый'],['medium','Средний'],['large','Крупный']]} /></SettingRow>
          <SettingRow label={english ? 'Text width' : 'Ширина текста'}><Segmented value={p.editorWidth} onChange={(v) => set('editorWidth', v as PreferencesState['editorWidth'])} options={english ? [['compact','Compact'],['normal','Normal'],['wide','Wide']] : [['compact','Компактная'],['normal','Обычная'],['wide','Широкая']]} /></SettingRow>
          <SettingRow label={english ? 'Autosave' : 'Автосохранение'}><Switch checked={p.autosave} onChange={(v) => set('autosave', v)} /></SettingRow>
          <SettingRow label={english ? 'Formatting toolbar' : 'Панель форматирования'}><Switch checked={p.showToolbar} onChange={(v) => set('showToolbar', v)} /></SettingRow>
          <SettingRow label={english ? 'Browser spellcheck' : 'Проверка орфографии браузером'}><Switch checked={p.spellcheck} onChange={(v) => set('spellcheck', v)} /></SettingRow>
        </SettingsSection>

        <SettingsSection icon={<Database />} title={english ? 'Data' : 'Данные'} description={english ? 'Backup or move your local journal.' : 'Резервная копия и перенос локального дневника.'}>
          <div className="settings-actions">
            <button className="settings-action" onClick={exportData}><Download size={18} />{english ? 'Export JSON' : 'Экспорт JSON'}</button>
            <button className="settings-action" onClick={() => fileRef.current?.click()}><Upload size={18} />{english ? 'Import JSON' : 'Импорт JSON'}</button>
            <input ref={fileRef} type="file" accept="application/json,.json" hidden onChange={importData} />
          </div>
        </SettingsSection>

        <SettingsSection danger icon={<Trash2 />} title={english ? 'Danger zone' : 'Опасная зона'} description={english ? 'These actions cannot be undone.' : 'Эти действия нельзя отменить.'}>
          <button className="settings-danger" onClick={() => setConfirmReset(true)}><RotateCcw size={18} />{english ? 'Delete all application data' : 'Удалить все данные приложения'}</button>
        </SettingsSection>
      </div>

      <div className="settings-about"><strong>Maybe Later Journal</strong><span>Browser MVP · v0.8</span><small>{english ? 'Your journal is stored locally in this browser.' : 'Ваш дневник хранится локально в этом браузере.'}</small></div>

      </div>
      <Modal open={confirmReset} title={english ? 'Delete all data?' : 'Удалить все данные?'} onClose={() => setConfirmReset(false)} footer={<><button className="modal__button" onClick={() => setConfirmReset(false)}>{english ? 'Cancel' : 'Отмена'}</button><button className="modal__button modal__button--danger" onClick={resetAll}>{english ? 'Delete' : 'Удалить'}</button></>}>
        <p>{english ? 'Entries, tags, calendar colors and settings will be deleted from this browser.' : 'Записи, теги, цвета календаря и настройки будут удалены из этого браузера.'}</p>
      </Modal>
    </main>
  );
}

function SettingsSection({ icon, title, description, children, danger = false }: { icon: React.ReactNode; title: string; description: string; children: React.ReactNode; danger?: boolean }) {
  return <section className={`settings-card ${danger ? 'settings-card--danger' : ''}`}><div className="settings-card__head"><span>{icon}</span><div><h2>{title}</h2><p>{description}</p></div></div><div className="settings-card__body">{children}</div></section>;
}
function SettingRow({ label, children }: { label: string; children: React.ReactNode }) { return <div className="setting-row"><span>{label}</span><div>{children}</div></div>; }
function Segmented({ value, options, onChange }: { value: string; options: readonly (readonly [string,string])[]; onChange: (value: string) => void }) { return <div className="segmented">{options.map(([key,label]) => <button key={key} className={value === key ? 'active' : ''} onClick={() => onChange(key)}>{label}</button>)}</div>; }
function Switch({ checked, onChange }: { checked: boolean; onChange: (value: boolean) => void }) { return <button type="button" className={`switch ${checked ? 'on' : ''}`} onClick={() => onChange(!checked)} aria-pressed={checked}><i /></button>; }
