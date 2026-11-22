import test from 'node:test';
import assert from 'node:assert/strict';
import { parseEntries, parseTags } from '../src/db/backupValidation.ts';

const entry = {
  id: 'entry-1',
  title: 'Test',
  content: 'Artificial data',
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

test('accepts valid entries and tags', () => {
  assert.deepEqual(parseEntries([entry]), [entry]);
  assert.deepEqual(parseTags([tag]), [tag]);
});

test('rejects non-array entries and tags', () => {
  assert.throws(() => parseEntries({}), /Некорректный массив/);
  assert.throws(() => parseTags(null), /Некорректный массив/);
});

test('rejects malformed entries', () => {
  assert.throws(() => parseEntries([{ ...entry, content: 123 }]));
  assert.throws(() => parseEntries([{ ...entry, tags: [42] }]));
  assert.throws(() => parseEntries([{ ...entry, id: '' }]));
});

test('rejects duplicate entry IDs', () => {
  assert.throws(() => parseEntries([entry, { ...entry }]), /Повторяющиеся ID/);
});

test('rejects malformed tags', () => {
  assert.throws(() => parseTags([{ ...tag, name: ' ' }]));
  assert.throws(() => parseTags([{ ...tag, color: null }]));
});

test('rejects duplicate tag IDs and names', () => {
  assert.throws(() => parseTags([tag, { ...tag, name: 'Other' }]));
  assert.throws(() => parseTags([tag, { ...tag, id: 'tag-2' }]));
});
