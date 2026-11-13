import { ChangeEvent, useEffect, useRef, useState } from 'react';
import { Database, Download, KeyRound, Languages, Monitor, Palette, RotateCcw, Settings2, Trash2, Upload } from 'lucide-react';
import { useAppDispatch, useAppSelector } from '../app/hooks';
import { setJournalName, setPreference, type PreferencesState } from '../features/preferences/preferencesSlice';
import { Modal } from '../components/Modal/Modal';
import './SettingsPage.css';
import { makeBackup, validateBackup, restoreBackup, BACKUP_KEYS } from '../db/backup';
import { isTauri, invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { save } from '@tauri-apps/plugin-dialog';
import { writeTextFile } from '@tauri-apps/plugin-fs';
import packageInfo from '../../package.json';
import { beginSqliteReset, cancelSqliteReset, waitForSqliteEntryWrites, waitForSqlitePreferencesWrites, waitForSqliteCalendarWrites } from '../app/store';
import { waitForSqliteTagWrites } from '../db/tagsRepository';

const APP_KEYS = BACKUP_KEYS;

export function SettingsPage() {
  const dispatch = useAppDispatch();
  const p = useAppSelector((state) => state.preferences);
  const english = p.language === 'en';
  const [appVersion, setAppVersion] = useState(packageInfo.version);
  useEffect(() => {
    if (isTauri()) void getVersion().then(setAppVersion).catch((error) => console.warn('Cannot read app version:', error));
  }, []);
  const fileRef = useRef<HTMLInputElement>(null);
  const [confirmReset, setConfirmReset] = useState(false);
  const [pendingImport, setPendingImport] = useState<ReturnType<typeof validateBackup> | null>(null);
  const [backupError, setBackupError] = useState('');
  const [backupSuccess, setBackupSuccess] = useState('');
  const [busy, setBusy] = useState(false);
  const [restoreCheck, setRestoreCheck] = useState('');
  const [diskCheck, setDiskCheck] = useState('');
  const [snapshotCheck, setSnapshotCheck] = useState('');
  const [confirmRealRestore, setConfirmRealRestore] = useState(false);
  const [securePassword, setSecurePassword] = useState('');
  const [passwordDialog, setPasswordDialog] = useState(false);
  const [oldPassword, setOldPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [passwordError, setPasswordError] = useState('');
  const [passwordNotice, setPasswordNotice] = useState('');
  const [restoreNotice] = useState(() => { const notice = sessionStorage.getItem('maybeLater.restoreNotice'); if (notice) sessionStorage.removeItem('maybeLater.restoreNotice'); return notice; });

  const set = <K extends keyof PreferencesState>(key: K, value: PreferencesState[K]) => {
    dispatch(setPreference({ key, value }));
  };

  const exportData = async () => {
    if (busy) return;
    setBusy(true);
    setBackupError('');
    setBackupSuccess('');
    try {
      const filename = `maybe-later-backup-${new Date().toISOString().slice(0, 10)}.json`;
      if (isTauri()) {
        // A user-selected destination is granted access by the dialog plugin.
        const path = await save({
          title: english ? 'Save journal backup' : 'Сохранить резервную копию дневника',
          defaultPath: filename,
          filters: [{ name: 'JSON', extensions: ['json'] }],
        });
        if (!path) return; // User cancelled; no error and no file is written.
        const content = await makeBackup();
        await writeTextFile(path, content);
        setBackupSuccess(english ? `Backup saved: ${path}` : `Резервная копия сохранена: ${path}`);
      } else {
        const content = await makeBackup();
        const blob = new Blob([content], { type: 'application/json' });
        const url = URL.createObjectURL(blob);
        try {
          const a = document.createElement('a');
          a.href = url;
          a.download = filename;
          document.body.appendChild(a);
          a.click();
          a.remove();
          setBackupSuccess(english ? 'Backup download started.' : 'Скачивание резервной копии началось.');
        } finally {
          setTimeout(() => URL.revokeObjectURL(url), 3000);
        }
      }
    } catch (error) {
      setBackupError(english ? `Backup failed: ${String(error)}` : `Не удалось сохранить резервную копию: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  };

  const importData = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (!file) return;
    try { setBackupError(''); setBackupSuccess(''); setRestoreCheck(''); setDiskCheck(''); setSnapshotCheck(''); setConfirmRealRestore(false); setPendingImport(validateBackup(await file.text())); }
    catch (error) { setBackupError(String(error)); }
  };

  const verifyImport = async () => {
    if (!pendingImport || !isTauri()) return;
    setBusy(true); setBackupError(''); setRestoreCheck(''); setDiskCheck(''); setSnapshotCheck('');
    try {
      const result = await invoke<string>('verify_sqlite_restore', { payload: { entries: pendingImport.entries, tags: pendingImport.tags, preferences: pendingImport.preferences, calendarColors: pendingImport.calendarColors } });
      setRestoreCheck(`✅ ${result}`);
    } catch (error) { setBackupError(`Проверка восстановления: ${String(error)}`); }
    finally { setBusy(false); }
  };

  const testOnDiskCopy = async () => {
    if (busy || !securePassword || !pendingImport || !restoreCheck || !isTauri()) return;
    setBusy(true); setBackupError(''); setDiskCheck(''); setSnapshotCheck('');
    try {
      const result = await invoke<string>('test_sqlite_restore_on_copy', { payload: { entries: pendingImport.entries, tags: pendingImport.tags, preferences: pendingImport.preferences, calendarColors: pendingImport.calendarColors }, password: securePassword });
      setDiskCheck(`✅ ${result}`);
    } catch (error) { setBackupError(`Тест на копии SQLite: ${String(error)}`); }
    finally { setBusy(false); }
  };

  const testSafetySnapshot = async () => {
    if (busy || !securePassword || !pendingImport || !restoreCheck || !diskCheck || !isTauri()) return;
    setBusy(true); setBackupError(''); setSnapshotCheck('');
    try {
      const result = await invoke<string>('test_sqlite_safety_snapshot', { password: securePassword });
      setSnapshotCheck(`✅ ${result}`);
    } catch (error) { setBackupError(`Проверка защитной копии: ${String(error)}`); }
    finally { setBusy(false); }
  };

  const doRealRestore = async () => {
    if (busy || !securePassword || !pendingImport || !restoreCheck || !diskCheck || !snapshotCheck || !isTauri()) return;
    setBusy(true); setBackupError('');
    try { await restoreBackup(pendingImport, securePassword); }
    catch (error) { setConfirmRealRestore(false); setDiskCheck(""); setSnapshotCheck(""); setSecurePassword(""); setBackupError(`Восстановление не выполнено: ${String(error)}`); }
    finally { setBusy(false); }
  };

  const closePasswordDialog = () => {
    if (busy) return;
    setPasswordDialog(false);
    setOldPassword(''); setNewPassword(''); setConfirmPassword(''); setPasswordError('');
  };

  const changePassword = async () => {
    if (busy || !isTauri()) return;
    if (!oldPassword || !newPassword || newPassword !== confirmPassword || oldPassword === newPassword) {
      setPasswordError(english ? 'Enter the current password and two matching, different new passwords.' : 'Введите текущий пароль и два одинаковых новых пароля, отличающихся от старого.');
      return;
    }
    setBusy(true); setPasswordError(''); setPasswordNotice('');
    try {
      // Block new writes and wait for autosave before the Rust session is closed.
      beginSqliteReset();
      await waitForSqliteEntryWrites();
      await waitForSqliteTagWrites();
      await waitForSqlitePreferencesWrites();
      await waitForSqliteCalendarWrites();
      await invoke('change_secure_database_password', { oldPassword, newPassword });
    } catch (error) {
      cancelSqliteReset();
      setPasswordError(String(error));
      setOldPassword(''); setNewPassword(''); setConfirmPassword('');
      setBusy(false);
      return;
    }

    // Password change succeeded. Lock the Rust session before showing UnlockScreen:
    // an already-unlocked session bypasses password verification in UnlockScreen.
    try {
      await invoke('lock_secure_database');
    } catch (error) {
      // Do not reload into a misleading unlock screen. The password has already
      // changed, so never present this as a failed password-change operation.
      cancelSqliteReset();
      setPasswordDialog(false);
      setPasswordNotice(english
        ? `Password changed, but the journal could not be locked automatically: ${String(error)}. Lock the journal manually before leaving it unattended.`
        : `Пароль изменён, но автоматически заблокировать дневник не удалось: ${String(error)}. Заблокируйте дневник вручную, прежде чем оставлять его без присмотра.`);
      setOldPassword(''); setNewPassword(''); setConfirmPassword('');
      setBusy(false);
      return;
    }

    setOldPassword(''); setNewPassword(''); setConfirmPassword('');
    // The backend is locked; a reload now requires a real password check.
    window.location.reload();
  };

  const resetAll = async () => {
    if (busy || (isTauri() && !securePassword)) return;
    setBusy(true);
    setBackupError('');
    try {
      if (isTauri()) {
        beginSqliteReset();
        await waitForSqliteEntryWrites();
        await waitForSqliteTagWrites();
        await waitForSqlitePreferencesWrites();
        await waitForSqliteCalendarWrites();
        const snapshotPath = await invoke<string>('reset_sqlite_data', { password: securePassword });
        // Remove only obsolete browser copies, not migration handover markers.
        APP_KEYS.forEach(key => localStorage.removeItem(key));
        sessionStorage.setItem('maybeLater.restoreNotice', `Данные удалены. Защитная копия SQLite: ${snapshotPath}`);
      } else {
        APP_KEYS.forEach(key => localStorage.removeItem(key));
      }
      window.location.reload();
    } catch (error) {
      cancelSqliteReset();
      setConfirmReset(false);
      setSecurePassword("");
      setBackupError(`Удаление не выполнено: ${String(error)}`);
      setBusy(false);
    }
  };

  return (
    <main className="settings-page">
      <header className="settings-header">
        <span>{english ? 'APPLICATION' : 'ПРИЛОЖЕНИЕ'}</span>
        <h1>{english ? 'Settings' : 'Настройки'}</h1>
        <p>{english ? 'Make the journal comfortable for the way you write.' : 'Настройте дневник так, чтобы им было удобно пользоваться именно вам.'}</p>
      </header>

      {restoreNotice && <p role="status" style={{padding:'12px 24px',overflowWrap:'anywhere'}}>{restoreNotice}</p>}
      <div className="settings-scroll page-scroll">
      <div className="settings-grid">
        <SettingsSection icon={<Palette />} title={english ? 'Appearance' : 'Внешний вид'} description={english ? 'Theme and workspace appearance.' : 'Тема и внешний вид рабочего пространства.'}>
          <SettingRow label={english ? 'Theme' : 'Тема'}>
            <Segmented value={p.theme} onChange={(v) => set('theme', v as PreferencesState['theme'])} options={english ? [['system','System'],['light','Light'],['dark','Dark'],['cream','Alternative']] : [['system','Системная'],['light','Светлая'],['dark','Тёмная'],['cream','Альтернативная']]} />
          </SettingRow>
          <SettingRow label={english ? 'Sidebar mountain background' : 'Горный фон Sidebar'}><Switch checked={p.theme !== 'cream' && p.sidebarBackground} disabled={p.theme === 'cream'} onChange={(v) => set('sidebarBackground', v)} /></SettingRow>
          <SettingRow label={english ? 'Translucent window (experimental)' : 'Полупрозрачное окно (экспериментально)'}><Switch checked={p.theme !== 'cream' && p.windowTransparency} disabled={p.theme === 'cream'} onChange={(v) => set('windowTransparency', v)} /></SettingRow>
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
            <button className="settings-action" disabled={busy} onClick={exportData}><Download size={18} />{english ? 'Export JSON' : 'Экспорт JSON'}</button>
            <button className="settings-action" disabled={busy} onClick={() => fileRef.current?.click()}><Upload size={18} />{english ? 'Import JSON' : 'Импорт JSON'}</button>
            <input ref={fileRef} type="file" accept="application/json,.json" hidden onChange={importData} />
          </div>
          {backupSuccess && <p role="status" style={{overflowWrap:'anywhere'}}>{backupSuccess}</p>}
          {backupError && <p role="alert" style={{color:'#ff9999',overflowWrap:'anywhere'}}>{backupError}</p>}
        </SettingsSection>

        {isTauri() && <SettingsSection icon={<KeyRound />} title={english ? 'Security' : 'Безопасность'} description={english ? 'Protect your encrypted journal.' : 'Защита зашифрованного дневника.'}>
          <button className="settings-action" disabled={busy} onClick={() => { setPasswordError(''); setPasswordDialog(true); }}><KeyRound size={18} />{english ? 'Change journal password' : 'Изменить пароль дневника'}</button>
          {passwordNotice && <p role="status">{passwordNotice}</p>}
        </SettingsSection>}

        <SettingsSection danger icon={<Trash2 />} title={english ? 'Danger zone' : 'Опасная зона'} description={english ? 'These actions cannot be undone.' : 'Эти действия нельзя отменить.'}>
          <button className="settings-danger" disabled={busy} onClick={() => setConfirmReset(true)}><RotateCcw size={18} />{english ? 'Delete all application data' : 'Удалить все данные приложения'}</button>
        </SettingsSection>
      </div>

      <div className="settings-about"><strong>Maybe Later</strong><span>{english ? 'Version' : 'Версия'} {appVersion}</span><small>{english ? 'Your journal is stored locally on this device.' : 'Ваш дневник хранится локально на этом устройстве.'}</small></div>

      </div>
      <Modal open={passwordDialog} title={english ? 'Change journal password' : 'Изменить пароль дневника'} onClose={closePasswordDialog} footer={<><button className="modal__button" disabled={busy} onClick={closePasswordDialog}>{english ? 'Cancel' : 'Отмена'}</button><button className="modal__button" disabled={busy || !oldPassword || !newPassword || !confirmPassword} onClick={changePassword}>{busy ? '...' : (english ? 'Change password' : 'Изменить пароль')}</button></>}>
        <p>{english ? 'The journal will be re-encrypted. The previous encrypted database is retained as a backup protected by the old password.' : 'Дневник будет перешифрован. Предыдущая зашифрованная база останется резервной копией, защищённой старым паролем.'}</p>
        <label>{english ? 'Current password' : 'Текущий пароль'}<input className="settings-input" type="password" autoComplete="current-password" value={oldPassword} disabled={busy} onChange={e => setOldPassword(e.target.value)} /></label>
        <label>{english ? 'New password' : 'Новый пароль'}<input className="settings-input" type="password" autoComplete="new-password" value={newPassword} disabled={busy} onChange={e => setNewPassword(e.target.value)} /></label>
        <label>{english ? 'Repeat new password' : 'Повторите новый пароль'}<input className="settings-input" type="password" autoComplete="new-password" value={confirmPassword} disabled={busy} onChange={e => setConfirmPassword(e.target.value)} /></label>
        {passwordError && <p role="alert" style={{overflowWrap:'anywhere'}}>{passwordError}</p>}
      </Modal>
      <Modal open={!!pendingImport} title={english ? 'Restore backup?' : 'Восстановить резервную копию?'} onClose={() => !busy && (setPendingImport(null), setConfirmRealRestore(false), setSecurePassword(""))} footer={<><button className="modal__button" disabled={busy} onClick={() => { setPendingImport(null); setConfirmRealRestore(false); setSecurePassword(""); }}>{english ? 'Cancel' : 'Отмена'}</button><button className="modal__button" disabled={busy || !isTauri()} onClick={verifyImport}>{busy ? '...' : (english ? 'Check' : 'Проверить')}</button><button className="modal__button modal__button--danger" disabled={busy || !restoreCheck || !diskCheck || !snapshotCheck || !isTauri()} onClick={() => setConfirmRealRestore(true)}>{english ? 'Restore…' : 'Восстановить…'}</button></>}>
        <p>{english ? 'First verify in memory, then test on a disk copy and verify a safety snapshot. Real restore REPLACES all current entries, tags and preferences and calendar colors (when included). A separate SQLite snapshot is saved before replacement.' : 'Сначала проверьте копию в памяти, затем выполните тест на дисковой копии и проверку защитного снимка. Настоящее восстановление ЗАМЕНИТ все текущие записи, теги и настройки и цвета календаря (если они есть в JSON). Перед заменой будет сохранена отдельная копия SQLite.'}</p>
        <p>{pendingImport?.entries.length ?? 0} {english ? 'entries' : 'записей'}, {pendingImport?.tags.length ?? 0} {english ? 'tags' : 'тегов'}, {pendingImport?.calendarColors ? Object.keys(pendingImport.calendarColors).length : (english ? 'unchanged' : 'без изменений')} {english ? 'calendar colors' : 'цветов календаря'}.</p>{restoreCheck && <p role="status" style={{overflowWrap:'anywhere'}}>{restoreCheck}</p>}
          {isTauri() && <label>Пароль дневника <input className="settings-input" type="password" autoComplete="current-password" value={securePassword} onChange={(e) => { setSecurePassword(e.target.value); setDiskCheck(""); setSnapshotCheck(""); setConfirmRealRestore(false); }} disabled={busy} /></label>}
        {restoreCheck && <button type="button" className="modal__button" disabled={busy || !isTauri() || !securePassword} onClick={testOnDiskCopy}>{english ? 'Test on SQLite copy' : 'Тест на копии SQLite'}</button>}
        {diskCheck && <button type="button" className="modal__button" disabled={busy || !isTauri() || !securePassword} onClick={testSafetySnapshot}>{english ? 'Verify safety snapshot' : 'Проверить защитную копию'}</button>}
        {snapshotCheck && <p role="status" style={{overflowWrap:'anywhere'}}>{snapshotCheck}</p>}
        {diskCheck && <p role="status" style={{overflowWrap:'anywhere'}}>{diskCheck}</p>}{backupError && <p role="alert" style={{color:'#ff9999',overflowWrap:'anywhere'}}>{backupError}</p>}
      </Modal>
      <Modal open={confirmRealRestore} title={english ? 'Confirm real restore' : 'Подтвердить настоящее восстановление'} onClose={() => !busy && setConfirmRealRestore(false)} footer={<><button className="modal__button" disabled={busy} onClick={() => setConfirmRealRestore(false)}>{english ? 'Cancel' : 'Отмена'}</button><button className="modal__button modal__button--danger" disabled={busy} onClick={doRealRestore}>{busy ? '...' : (english ? 'Replace SQLite data' : 'Заменить данные SQLite')}</button></>}>
        <p>{english ? 'This will replace entries, tags, preferences and calendar colors (when included) in one SQLite transaction. Existing entries not in the backup will disappear. A safety snapshot of the old database will be created automatically.' : 'Записи, теги и настройки и цвета календаря (если они есть в JSON) будут заменены в одной транзакции SQLite. Записи, которых нет в копии, исчезнут из дневника. Старая база будет автоматически сохранена отдельным файлом.'}</p>
        <p><strong>{pendingImport?.entries.length ?? 0} {english ? 'entries' : 'записей'}, {pendingImport?.tags.length ?? 0} {english ? 'tags' : 'тегов'}</strong></p>
      </Modal>
      <Modal open={confirmReset} title={english ? 'Delete all data?' : 'Удалить все данные?'} onClose={() => !busy && (setConfirmReset(false), setSecurePassword(""))} footer={<><button className="modal__button" disabled={busy} onClick={() => { setConfirmReset(false); setSecurePassword(""); }}>{english ? 'Cancel' : 'Отмена'}</button><button className="modal__button modal__button--danger" disabled={busy} onClick={resetAll}>{busy ? '…' : (english ? 'Delete' : 'Удалить')}</button></>}>
        <p>{english ? (isTauri() ? 'Entries, tags, calendar colors and settings will be deleted from SQLite. A recovery snapshot will be saved first.' : 'Entries, tags, calendar colors and settings will be deleted from this browser.') : (isTauri() ? 'Записи, теги, цвета календаря и настройки будут удалены из SQLite. Перед удалением будет создана защитная копия.' : 'Записи, теги, цвета календаря и настройки будут удалены из этого браузера.')}</p>
          {isTauri() && <label>Пароль дневника <input className="settings-input" type="password" autoComplete="current-password" value={securePassword} onChange={(e) => { setSecurePassword(e.target.value); setDiskCheck(""); setSnapshotCheck(""); setConfirmRealRestore(false); }} disabled={busy} /></label>}
      </Modal>
    </main>
  );
}

function SettingsSection({ icon, title, description, children, danger = false }: { icon: React.ReactNode; title: string; description: string; children: React.ReactNode; danger?: boolean }) {
  return <section className={`settings-card ${danger ? 'settings-card--danger' : ''}`}><div className="settings-card__head"><span>{icon}</span><div><h2>{title}</h2><p>{description}</p></div></div><div className="settings-card__body">{children}</div></section>;
}
function SettingRow({ label, children }: { label: string; children: React.ReactNode }) { return <div className="setting-row"><span>{label}</span><div>{children}</div></div>; }
function Segmented({ value, options, onChange }: { value: string; options: readonly (readonly [string,string])[]; onChange: (value: string) => void }) { return <div className="segmented">{options.map(([key,label]) => <button key={key} className={value === key ? 'active' : ''} onClick={() => onChange(key)}>{label}</button>)}</div>; }
function Switch({ checked, onChange, disabled = false }: { checked: boolean; onChange: (value: boolean) => void; disabled?: boolean }) { return <button type="button" className={`switch ${checked ? 'on' : ''}`} onClick={() => onChange(!checked)} aria-pressed={checked} disabled={disabled}><i /></button>; }
