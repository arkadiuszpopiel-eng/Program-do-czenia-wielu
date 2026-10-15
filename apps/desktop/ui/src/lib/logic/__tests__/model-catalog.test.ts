import { describe, expect, it } from 'vitest';
import type { ModelItem } from '../../api/types-models';
import { SORT_ORDERS, diskUsage, searchItems, sortItems } from '../model-catalog';

const item = (patch: Partial<ModelItem>): ModelItem => ({
  id: 'x',
  kind: 'llm',
  name: 'X',
  license: 'MIT',
  source: 'https://example.invalid/x',
  size_bytes: 0,
  target: 'C:/modele/x',
  files: [],
  state: 'missing',
  pinned: true,
  confirmed: true,
  downloadable: true,
  note: { pl: '', en: '' },
  progress: null,
  error: null,
  active: false,
  card: null,
  ...patch,
});

const ITEMS: readonly ModelItem[] = [
  item({ id: 'llm-bielik', name: 'Bielik 4.5B', license: 'Apache-2.0', size_bytes: 3000 }),
  item({
    id: 'stt-whisper',
    kind: 'stt',
    name: 'Whisper turbo',
    size_bytes: 1500,
    state: 'installed',
  }),
  item({
    id: 'tts-piper-gosia',
    kind: 'tts',
    name: 'Głos Gosia',
    size_bytes: 60,
    state: 'external',
  }),
  item({
    id: 'sidecar-llama',
    kind: 'sidecar',
    name: 'Silnik llama',
    size_bytes: 400,
    downloadable: false,
  }),
  item({ id: 'vad', kind: 'vad', name: 'Żaba VAD', size_bytes: 60, state: 'failed' }),
];

const ids = (list: readonly ModelItem[]) => list.map((i) => i.id);

describe('katalog modeli', () => {
  it('szuka po nazwie, identyfikatorze i licencji, bez polskich znaków i wielkości liter', () => {
    expect(searchItems(ITEMS, '')).toBe(ITEMS);
    expect(searchItems(ITEMS, '   ')).toBe(ITEMS);
    expect(ids(searchItems(ITEMS, 'glos'))).toEqual(['tts-piper-gosia']);
    expect(ids(searchItems(ITEMS, 'ZABA'))).toEqual(['vad']);
    expect(ids(searchItems(ITEMS, 'apache'))).toEqual(['llm-bielik']);
    expect(ids(searchItems(ITEMS, 'piper gosia'))).toEqual(['tts-piper-gosia']);
    expect(ids(searchItems(ITEMS, 'llama'))).toEqual(['sidecar-llama']);
    expect(searchItems(ITEMS, 'bielik whisper')).toEqual([]);
  });

  it('sortuje po nazwie i rozmiarze, nie zmieniając listy wejściowej', () => {
    expect(SORT_ORDERS).toEqual(['catalog', 'name', 'size_desc', 'size_asc']);
    expect(sortItems(ITEMS, 'catalog')).toBe(ITEMS);
    expect(ids(sortItems(ITEMS, 'name'))).toEqual([
      'llm-bielik',
      'tts-piper-gosia',
      'sidecar-llama',
      'stt-whisper',
      'vad',
    ]);
    expect(ids(sortItems(ITEMS, 'size_desc')).slice(0, 3)).toEqual([
      'llm-bielik',
      'stt-whisper',
      'sidecar-llama',
    ]);
    // Równy rozmiar — kolejność po nazwie, w obu kierunkach.
    expect(ids(sortItems(ITEMS, 'size_asc')).slice(0, 2)).toEqual(['tts-piper-gosia', 'vad']);
    expect(ids(sortItems(ITEMS, 'size_desc')).slice(3)).toEqual(['tts-piper-gosia', 'vad']);
    expect(ids(ITEMS)[0]).toBe('llm-bielik');
  });

  it('liczy miejsce: na dysku i do pobrania (bez pozycji ręcznych i w błędzie)', () => {
    expect(diskUsage(ITEMS)).toEqual({
      installedCount: 2,
      installedBytes: 1560,
      availableCount: 1,
      availableBytes: 3000,
    });
    expect(diskUsage([])).toEqual({
      installedCount: 0,
      installedBytes: 0,
      availableCount: 0,
      availableBytes: 0,
    });
  });
});
