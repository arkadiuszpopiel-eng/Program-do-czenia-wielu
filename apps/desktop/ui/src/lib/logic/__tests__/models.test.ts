import { describe, expect, it } from 'vitest';
import type { ModelItem, ReindexView } from '../../api/types-models';
import {
  embedderChoices,
  filterItems,
  groupedHash,
  itemActions,
  progressPercent,
  reindexPercent,
  stateGroup,
  trustHashes,
  upsertItem,
} from '../models';

const base: ModelItem = {
  id: 'x',
  kind: 'stt',
  name: 'X',
  license: 'MIT',
  source: 'https://example.invalid',
  size_bytes: 100,
  target: 'C:\\x',
  files: [
    {
      name: 'a.bin',
      url: 'https://e/a',
      size_bytes: 50,
      pinned_sha256: 'a'.repeat(64),
      sha256: null,
    },
    {
      name: 'b.bin',
      url: 'https://e/b',
      size_bytes: 50,
      pinned_sha256: null,
      sha256: 'b'.repeat(64),
    },
  ],
  state: 'missing',
  pinned: false,
  confirmed: false,
  downloadable: true,
  note: { pl: 'n', en: 'n' },
  progress: null,
  error: null,
  active: false,
};
const item = (patch: Partial<ModelItem>): ModelItem => ({ ...base, ...patch });

describe('logika: modele i silniki', () => {
  it('akcje zależą od stanu', () => {
    expect(itemActions(item({}))).toMatchObject({ download: true, remove: false, verify: false });
    const partial = { file: 'a.bin', done: 10, total: 50 };
    expect(itemActions(item({ state: 'paused', progress: partial }))).toMatchObject({
      resume: true,
      download: false,
      remove: true,
    });
    expect(itemActions(item({ state: 'downloading' }))).toMatchObject({
      cancel: true,
      remove: false,
    });
    expect(itemActions(item({ state: 'needs_trust' })).trust).toBe(true);
    const embed = item({ kind: 'embed', state: 'installed' });
    expect(itemActions(embed)).toMatchObject({ activate: true, verify: true, remove: true });
    expect(itemActions({ ...embed, active: true })).toMatchObject({
      activate: false,
      remove: false,
    });
    expect(itemActions(item({ downloadable: false })).download).toBe(false);
  });

  it('filtry, postęp, hashe do zgody', () => {
    const list = [item({ id: 'a' }), item({ id: 'b', kind: 'tts', state: 'installed' })];
    expect(filterItems(list, 'tts', 'all').map((i) => i.id)).toEqual(['b']);
    expect(filterItems(list, 'all', 'installed').map((i) => i.id)).toEqual(['b']);
    expect(filterItems(list, 'all', 'available').map((i) => i.id)).toEqual(['a']);
    expect(stateGroup('needs_trust')).toBe('attention');
    expect(stateGroup('external')).toBe('installed');
    expect(progressPercent(item({ progress: { file: 'a', done: 25, total: 50 } }))).toBe(50);
    expect(progressPercent(item({ progress: { file: 'a', done: 25, total: null } }))).toBeNull();
    expect(trustHashes(item({}))).toEqual({ 'b.bin': 'b'.repeat(64) });
    expect(groupedHash('0123456789abcdef')).toBe('01234567 89abcdef');
    const updated = upsertItem(list, item({ id: 'a', state: 'queued' }));
    expect(updated.map((i) => i.state)).toEqual(['queued', 'installed']);
    expect(upsertItem(list, item({ id: 'c' }))).toHaveLength(3);
    expect(
      embedderChoices([...list, item({ id: 'e', kind: 'embed', state: 'installed' })]),
    ).toHaveLength(1);
  });

  it('postęp przebudowy', () => {
    const view: ReindexView = {
      running: true,
      embedder: 'e5/384',
      databases: 2,
      rebuilt: 1,
      embedded: 10,
      done: 30,
      total: 120,
      failed: 0,
      cancelled: false,
      finished: false,
    };
    expect(reindexPercent(view)).toBe(25);
    expect(reindexPercent({ ...view, running: false })).toBeNull();
  });
});
