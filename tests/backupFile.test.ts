import test from 'node:test';
import assert from 'node:assert/strict';
import { BACKUP_KEYS, validateBackup } from '../src/db/backupValidation.ts';

const entry = {
  id: 'entry-1',
  title: 'Test',
  content: 'Artificial test data',
  date: '2026-01-01',
  tags: ['tag-1'],
  createdAt: '2026-01-01T10:00:00Z',
  updatedAt: '2026-01-01T10:00:00Z'
};

const tag = {
  id: 'tag-1',
  name: 'Test tag',
  color: '#00AA00'
};

const reference = {
  language: 'ru',
  theme: 'dark'
};

function backup(data: Record<string, unknown>, app = 'Maybe Later Journal'): string {
  return JSON.stringify({ app, version: 2, data });
}

function validData(): Record<string, unknown> {
  return {
    [BACKUP_KEYS[0]]: JSON.stringify([entry]),
    [BACKUP_KEYS[1]]: JSON.stringify([tag]),
    [BACKUP_KEYS[2]]: JSON.stringify({ '2026-01-01': '#123456' }),
    [BACKUP_KEYS[3]]: JSON.stringify({ language: 'en' })
  };
}

test('accepts a complete valid backup', () => {
  const result = validateBackup(backup(validData()), reference);
  assert.equal(result.entries.length, 1);
  assert.equal(result.tags.length, 1);
  assert.equal(result.preferences?.language, 'en');
  assert.equal(result.preferences?.theme, 'dark');
  assert.deepEqual(result.calendarColors, { '2026-01-01': '#123456' });
});

test('accepts legacy backup without preferences and calendar colors', () => {
  const data = validData();
  delete data[BACKUP_KEYS[2]];
  delete data[BACKUP_KEYS[3]];
  const result = validateBackup(backup(data), reference);
  assert.equal(result.preferences, null);
  assert.equal(result.calendarColors, null);
});

test('rejects invalid JSON and wrong application', () => {
  assert.throws(() => validateBackup('{broken', reference));
  assert.throws(() => validateBackup(backup(validData(), 'Other App'), reference));
});

test('rejects missing entries or tags', () => {
  const data = validData();
  delete data[BACKUP_KEYS[0]];
  assert.throws(() => validateBackup(backup(data), reference));
});

test('rejects duplicate entry IDs in backup', () => {
  const data = validData();
  data[BACKUP_KEYS[0]] = JSON.stringify([entry, { ...entry }]);
  assert.throws(() => validateBackup(backup(data), reference));
});

test('rejects invalid calendar colors', () => {
  const data = validData();
  data[BACKUP_KEYS[2]] = JSON.stringify({ 'invalid-date': '#123456' });
  assert.throws(() => validateBackup(backup(data), reference));
});

test('rejects unknown or incorrectly typed preferences', () => {
  const data = validData();
  data[BACKUP_KEYS[3]] = JSON.stringify({ unknownSetting: true });
  assert.throws(() => validateBackup(backup(data), reference));

  data[BACKUP_KEYS[3]] = JSON.stringify({ language: 123 });
  assert.throws(() => validateBackup(backup(data), reference));
});

test('rejects non-string backup fields', () => {
  const data = validData();
  data[BACKUP_KEYS[0]] = [entry];
  assert.throws(() => validateBackup(backup(data), reference));
});

test('rejects missing tags', () => {
  const data = validData();
  delete data[BACKUP_KEYS[1]];
  assert.throws(() => validateBackup(backup(data), reference));
});

test('accepts null optional fields', () => {
  const data = validData();
  data[BACKUP_KEYS[2]] = null;
  data[BACKUP_KEYS[3]] = null;
  const result = validateBackup(backup(data), reference);
  assert.equal(result.calendarColors, null);
  assert.equal(result.preferences, null);
});

test('rejects malformed JSON inside backup fields', () => {
  for (const key of BACKUP_KEYS) {
    const data = validData();
    data[key] = '{broken';
    assert.throws(() => validateBackup(backup(data), reference));
  }
});

test('rejects missing backup data object', () => {
  assert.throws(() =>
    validateBackup(JSON.stringify({ app: 'Maybe Later Journal', version: 2 }), reference)
  );
});
