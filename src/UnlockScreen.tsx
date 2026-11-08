import { useEffect, useState, type FormEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './UnlockScreen.css';

type DatabaseStatus = 'missing' | 'plaintext' | 'existing' | 'migration_pending' | null;
type ScreenMode = 'unlock' | 'create' | 'migrate';

export default function UnlockScreen({ onUnlocked }: { onUnlocked: () => void }) {
  const [password, setPassword] = useState('');
  const [databaseStatus, setDatabaseStatus] = useState<DatabaseStatus>(null);
  const [checkingDatabase, setCheckingDatabase] = useState(true);
  const [mode, setMode] = useState<ScreenMode>('unlock');
  const [confirmation, setConfirmation] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [passwordVisible, setPasswordVisible] = useState(false);
  const [confirmationVisible, setConfirmationVisible] = useState(false);

  useEffect(() => {
    let active = true;

    invoke<string>('get_secure_database_presence')
      .then(status => {
        if (!active) return;

        if (
          status !== 'missing' &&
          status !== 'plaintext' &&
          status !== 'existing' &&
          status !== 'migration_pending'
        ) {
          throw new Error('Неизвестное состояние дневника');
        }

        setDatabaseStatus(status);

        if (status === 'missing') {
          setMode('create');
        } else if (status === 'plaintext' || status === 'migration_pending') {
          setMode('migrate');
        } else {
          setMode('unlock');
        }
      })
      .catch(reason => {
        if (active) {
          setError('Не удалось проверить наличие дневника: ' + String(reason));
        }
      })
      .finally(() => {
        if (active) setCheckingDatabase(false);
      });

    return () => {
      active = false;
    };
  }, []);

  const isResumingMigration = databaseStatus === 'migration_pending';
  const needsConfirmation =
    mode === 'create' || (mode === 'migrate' && !isResumingMigration);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (busy || checkingDatabase || databaseStatus === null || !password) {
      return;
    }

    const statusMatches =
      mode === 'create'
        ? databaseStatus === 'missing'
        : mode === 'migrate'
          ? databaseStatus === 'plaintext' || databaseStatus === 'migration_pending'
          : databaseStatus === 'existing';

    if (!statusMatches) {
      setError('Действие недоступно для текущего состояния дневника');
      return;
    }

    if (needsConfirmation && password !== confirmation) {
      setError('Пароли не совпадают');
      return;
    }

    setBusy(true);
    setError('');

    try {
      if (mode === 'create') {
        await invoke('create_secure_database', { password });
      } else if (mode === 'migrate') {
        await invoke('migrate_plaintext_database', { password });
      } else {
        // В dev-режиме Vite может перезагрузить frontend, не перезапуская
        // Rust-процесс. В этом случае SQLite-сессия может уже оставаться
        // разблокированной, и повторный unlock_secure_database закономерно
        // завершится ошибкой "Разблокировка сейчас запрещена".
        const sessionStatus = await invoke<string>('get_secure_session_status');

        if (sessionStatus === 'Locked') {
          await invoke('unlock_secure_database', { password });
        } else if (sessionStatus !== 'Unlocked') {
          throw new Error(
            `Нельзя разблокировать дневник: состояние сессии «${sessionStatus}»`,
          );
        }
        // Если сессия уже Unlocked, повторно открывать соединение не нужно.
      }

      setPassword('');
      setConfirmation('');
      onUnlocked();
    } catch (reason) {
      const action =
        mode === 'create'
          ? 'создать дневник'
          : mode === 'migrate'
            ? isResumingMigration
              ? 'продолжить миграцию дневника'
              : 'перевести дневник на шифрование'
            : 'разблокировать дневник';

      setError(`Не удалось ${action}: ${String(reason)}`);
    } finally {
      setBusy(false);
    }
  }

  const title =
    mode === 'create'
      ? 'Создать дневник'
      : mode === 'migrate'
        ? isResumingMigration
          ? 'Продолжить шифрование'
          : 'Защитить существующий дневник'
        : 'Дневник заблокирован';

  const description =
    mode === 'create'
      ? 'Задайте пароль, который будет защищать ваши записи.'
      : mode === 'migrate'
        ? isResumingMigration
          ? 'Введите пароль, заданный при начале миграции, чтобы безопасно продолжить.'
          : 'Существующий дневник будет зашифрован. Исходная база сохранится в резервной копии.'
        : 'Введите пароль, чтобы открыть личные записи.';

  const submitLabel =
    busy
      ? mode === 'create'
        ? 'Создание…'
        : mode === 'migrate'
          ? 'Перенос и шифрование…'
          : 'Проверка…'
      : mode === 'create'
        ? 'Создать защищённый дневник'
        : mode === 'migrate'
          ? isResumingMigration
            ? 'Продолжить миграцию'
            : 'Зашифровать и продолжить'
          : 'Разблокировать дневник';

  const actionUnavailable =
    mode === 'create'
      ? databaseStatus !== 'missing'
      : mode === 'migrate'
        ? databaseStatus !== 'plaintext' && databaseStatus !== 'migration_pending'
        : databaseStatus !== 'existing';

  return (
    <main className="unlock-screen">
      <div className="unlock-screen__decoration unlock-screen__decoration--one" aria-hidden="true" />
      <div className="unlock-screen__decoration unlock-screen__decoration--two" aria-hidden="true" />

      <section className="unlock-card" aria-labelledby="unlock-title">
        <div className="unlock-brand">
          <div className="unlock-brand__mark" aria-hidden="true">
            <svg viewBox="0 0 32 32" fill="none">
              <path d="M5.5 7.8c4.1-.9 7.6-.1 10.5 2.2v16c-2.9-2.3-6.4-3.1-10.5-2.2V7.8Z" />
              <path d="M26.5 7.8c-4.1-.9-7.6-.1-10.5 2.2v16c2.9-2.3 6.4-3.1 10.5-2.2V7.8Z" />
              <path d="M16 10v16" />
            </svg>
          </div>
          <span>Maybe Later</span>
        </div>

        <div className="unlock-card__intro">
          <div className="unlock-card__eyebrow">
            {mode === 'unlock' ? 'ЛИЧНОЕ ПРОСТРАНСТВО' : 'НАСТРОЙКА ДНЕВНИКА'}
          </div>
          <h1 id="unlock-title">{title}</h1>
          <p>{description}</p>
        </div>

        {checkingDatabase && (
          <div className="unlock-notice" role="status">
            <span className="unlock-spinner" aria-hidden="true" />
            Проверяем состояние дневника…
          </div>
        )}

        {databaseStatus === 'migration_pending' && (
          <div className="unlock-notice unlock-notice--warning" role="alert">
            Обнаружена незавершённая миграция. Не удаляйте файлы дневника и резервные
            копии. Для продолжения используйте тот же пароль, который задавали при
            начале миграции.
          </div>
        )}

        {databaseStatus === 'plaintext' && (
          <div className="unlock-notice unlock-notice--info" role="status">
            Данные будут перенесены в зашифрованную базу. Исходная SQLite-база
            останется в резервной копии. Не закрывайте приложение во время переноса.
          </div>
        )}

        <form className="unlock-form" onSubmit={submit}>
          <label htmlFor="journal-password">
            {mode === 'migrate'
              ? isResumingMigration
                ? 'Пароль, заданный при начале миграции'
                : 'Новый пароль дневника'
              : mode === 'create'
                ? 'Придумайте пароль'
                : 'Пароль'}
          </label>

          <div className="unlock-input-wrap">
            <input
              id="journal-password"
              className="unlock-input"
              type={passwordVisible ? 'text' : 'password'}
              autoComplete={
                mode === 'unlock' || isResumingMigration
                  ? 'current-password'
                  : 'new-password'
              }
              value={password}
              onChange={event => setPassword(event.target.value)}
              disabled={busy}
              placeholder="Введите пароль"
              required
              autoFocus
            />
            <button
              className="unlock-visibility"
              type="button"
              onClick={() => setPasswordVisible(value => !value)}
              disabled={busy}
              aria-label={passwordVisible ? 'Скрыть пароль' : 'Показать пароль'}
              title={passwordVisible ? 'Скрыть пароль' : 'Показать пароль'}
            >
              {passwordVisible ? (
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path d="M3 3l18 18M10.6 10.7a2 2 0 0 0 2.7 2.7" />
                  <path d="M9.9 5.2A10.8 10.8 0 0 1 12 5c5.2 0 8.8 5 9.5 6-.3.5-1.5 2.2-3.7 3.7M6.2 6.2C3.9 7.7 2.8 9.5 2.5 11c.7 1 4.3 6 9.5 6 1 0 1.9-.2 2.8-.5" />
                </svg>
              ) : (
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path d="M2.5 12s3.4-6 9.5-6 9.5 6 9.5 6-3.4 6-9.5 6-9.5-6-9.5-6Z" />
                  <circle cx="12" cy="12" r="2.5" />
                </svg>
              )}
            </button>
          </div>

          {needsConfirmation && (
            <>
              <label htmlFor="journal-password-confirmation">Повторите пароль</label>
              <div className="unlock-input-wrap">
                <input
                  id="journal-password-confirmation"
                  className="unlock-input"
                  type={confirmationVisible ? 'text' : 'password'}
                  autoComplete="new-password"
                  value={confirmation}
                  onChange={event => setConfirmation(event.target.value)}
                  disabled={busy}
                  placeholder="Введите пароль ещё раз"
                  required
                />
                <button
                  className="unlock-visibility"
                  type="button"
                  onClick={() => setConfirmationVisible(value => !value)}
                  disabled={busy}
                  aria-label={confirmationVisible ? 'Скрыть пароль' : 'Показать пароль'}
                  title={confirmationVisible ? 'Скрыть пароль' : 'Показать пароль'}
                >
                  {confirmationVisible ? 'Скрыть' : 'Показать'}
                </button>
              </div>
            </>
          )}

          {error && (
            <div className="unlock-error" role="alert">
              <span aria-hidden="true">!</span>
              {error}
            </div>
          )}

          <button
            className="unlock-submit"
            type="submit"
            disabled={
              busy ||
              checkingDatabase ||
              !password ||
              actionUnavailable ||
              (needsConfirmation && password !== confirmation)
            }
          >
            {busy && <span className="unlock-spinner unlock-spinner--light" aria-hidden="true" />}
            {submitLabel}
            {!busy && <span aria-hidden="true">→</span>}
          </button>
        </form>

        <div className="unlock-card__footer">
          <span className="unlock-card__lock" aria-hidden="true">⌑</span>
          Ваши записи хранятся только на этом устройстве
        </div>
      </section>

      <div className="unlock-screen__version">MAYBE LATER JOURNAL</div>
    </main>
  );
}
