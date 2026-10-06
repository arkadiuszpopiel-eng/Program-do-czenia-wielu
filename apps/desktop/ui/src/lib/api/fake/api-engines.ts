// Atrapa: menedżer modeli i silników. Deterministyczna (zegar atrapy): pobieranie 4 krokami co
// 250 ms (zdarzenia `ModelProgress` + `ModelChanged`), limit 2 równoległych (reszta w kolejce),
// anulowanie → „wstrzymane” z plikiem częściowym, pozycje bez przypiętego hasha → karta zgody
// z policzonym SHA-256, weryfikacja, usuwanie (nie aktywnego embeddera), embedder wyszukiwania
// i przebudowa wektorów (zdarzenia `ReindexStatus`). Scenariusz „offline”: pobieranie zawodzi.
import type { EnginesApi } from '../client-models';
import type {
  EmbedderView,
  ModelItem,
  ModelItemKind,
  ModelItemState,
  ModelsView,
  ReindexView,
  TrustedHashes,
} from '../types-models';
import type { FakeCore } from './core';

export const ENGINE_STEP_MS = 250;
const STEPS = 4;
const PARALLEL = 2;
const MIB = 1024 * 1024;
const LEXICAL = 'lexical';
const LEXICAL_INDEX = 'alfa-lexical-hash-v1/256';

/** Deterministyczny „SHA-256” atrapy (64 znaki hex z identyfikatora pliku). */
export function fakeSha(seed: string): string {
  let h = 2166136261;
  let out = '';
  while (out.length < 64) {
    for (const c of seed + out.length) h = Math.imul(h ^ c.charCodeAt(0), 16777619) >>> 0;
    out += h.toString(16).padStart(8, '0');
  }
  return out.slice(0, 64);
}

type Seed = [
  id: string,
  kind: ModelItemKind,
  name: string,
  license: string,
  mib: number,
  files: readonly string[],
  pinned: boolean,
  confirmed: boolean,
  note: readonly [string, string],
];

const SEEDS: readonly Seed[] = [
  [
    'bielik-4.5b-v3.0-instruct-q8_0',
    'llm',
    'Bielik 4.5B v3.0 Instruct (Q8_0)',
    'Apache-2.0',
    4826,
    ['Bielik-4.5B-v3.0-Instruct.Q8_0.gguf'],
    false,
    false,
    [
      'Lokalny model rozmowy (llama.cpp). Wymaga sidecara llama-server.',
      'Local chat model (llama.cpp). Requires the llama-server sidecar.',
    ],
  ],
  [
    'multilingual-e5-small',
    'embed',
    'Multilingual E5 small (ONNX fp32)',
    'MIT',
    488,
    ['onnx/model.onnx', 'tokenizer.json'],
    false,
    false,
    ['Embedder wyszukiwania semantycznego (pamięć F7).', 'Semantic search embedder (F7 memory).'],
  ],
  [
    'whisper-large-v3-turbo-q5_0',
    'stt',
    'Whisper large-v3-turbo-q5_0',
    'MIT',
    547,
    ['ggml-large-v3-turbo-q5_0.bin'],
    false,
    false,
    [
      'Rozpoznawanie mowy. Wymaga sidecara whisper-server.',
      'Speech recognition. Requires the whisper-server sidecar.',
    ],
  ],
  [
    'silero-vad',
    'vad',
    'Silero VAD 6.2.3 (op18, bez If)',
    'MIT',
    11,
    ['silero_vad-6.2.3-py3-none-any.whl'],
    true,
    true,
    [
      'Wykrywanie mowy. Z paczki PyPI, hash przypięty.',
      'Voice activity detection. From the PyPI package, pinned hash.',
    ],
  ],
  [
    'openwakeword-features',
    'wake',
    'openWakeWord 0.5.1: melspektrogram + embedding',
    'Apache-2.0',
    16,
    ['openwakeword-0.5.1-py3-none-any.whl'],
    true,
    true,
    [
      'Cechy słów wywoławczych; klasyfikator PL — własny trening.',
      'Wake-word features; the PL classifier needs own training.',
    ],
  ],
  [
    'sidecar-llama-vulkan',
    'sidecar',
    'llama-server (vulkan)',
    'MIT',
    40,
    ['llama-vulkan.zip'],
    false,
    false,
    [
      'Serwer modeli lokalnych (127.0.0.1). Wersja do potwierdzenia.',
      'Local model server (127.0.0.1). Version to be confirmed.',
    ],
  ],
  [
    'sidecar-pocket-tts',
    'sidecar',
    'Pocket TTS PL (wrapper JSON-lines)',
    'CC-BY-4.0',
    0,
    [],
    false,
    false,
    [
      'Instalacja ręczna: wrapper budowany osobno.',
      'Manual install: the wrapper is built separately.',
    ],
  ],
];

function seedItem([id, kind, name, license, mib, files, pinned, confirmed, note]: Seed): ModelItem {
  const size = mib * MIB;
  return {
    id,
    kind,
    name,
    license,
    source: 'https://huggingface.co/',
    size_bytes: size,
    target: `%LOCALAPPDATA%\\Alfa\\${kind === 'sidecar' ? 'sidecars' : 'models'}\\${id}`,
    files: files.map((f) => ({
      name: f,
      url: `https://example.invalid/${f}`,
      size_bytes: Math.round(size / files.length),
      pinned_sha256: pinned ? fakeSha(f) : null,
      sha256: null,
    })),
    state: 'missing',
    pinned,
    confirmed,
    downloadable: files.length > 0,
    note: { pl: note[0], en: note[1] },
    progress: null,
    error: null,
    active: false,
  };
}

export class FakeEngines {
  private items = new Map<string, ModelItem>(SEEDS.map((s) => [s[0], seedItem(s)]));
  private timers = new Map<string, number>();
  private embedder: EmbedderView = {
    configured: 'multilingual-e5-small',
    active: LEXICAL,
    index_id: LEXICAL_INDEX,
    dims: 256,
    error: null,
  };
  private reindex: ReindexView = {
    running: false,
    embedder: LEXICAL_INDEX,
    databases: 0,
    rebuilt: 0,
    embedded: 0,
    done: 0,
    total: 0,
    failed: 0,
    cancelled: false,
    finished: true,
  };
  private reindexTimer: number | null = null;

  constructor(private readonly core: FakeCore) {}

  private get(id: string): ModelItem {
    const item = this.items.get(id);
    if (!item) throw new Error(`Nie znaleziono: pozycja katalogu „${id}”.`);
    return item;
  }

  private set(id: string, patch: Partial<ModelItem>): ModelItem {
    const item = { ...this.get(id), ...patch };
    this.items.set(id, item);
    this.core.emit([{ type: 'ModelChanged', item }]);
    return item;
  }

  private running(): number {
    return [...this.items.values()].filter((i) => i.state === 'downloading').length;
  }

  private pump(): void {
    for (const item of this.items.values()) {
      if (this.running() >= PARALLEL) return;
      if (item.state === 'queued') this.start(item.id);
    }
  }

  private start(id: string): void {
    const item = this.get(id);
    const done = item.progress?.done ?? 0;
    this.set(id, { state: 'downloading', error: null });
    this.tick(id, done);
  }

  private tick(id: string, done: number): void {
    const item = this.get(id);
    const file = item.files[0]?.name ?? '';
    if (this.core.scenario === 'offline') {
      this.timers.delete(id);
      this.set(id, { state: 'failed', error: 'sieć: brak połączenia (atrapa)', progress: null });
      this.pump();
      return;
    }
    if (done < item.size_bytes) {
      const progress = { file, done, total: item.size_bytes };
      this.items.set(id, { ...item, progress });
      this.core.emit([{ type: 'ModelProgress', item_id: id, ...progress }]);
      const next = Math.min(item.size_bytes, done + item.size_bytes / STEPS);
      this.timers.set(
        id,
        this.core.scheduler.setTimeout(() => this.tick(id, next), ENGINE_STEP_MS),
      );
      return;
    }
    this.timers.delete(id);
    const files = item.files.map((f) => ({
      ...f,
      sha256: f.pinned_sha256 ?? fakeSha(`tofu:${f.name}`),
    }));
    if (item.pinned) this.install(id, files);
    else this.set(id, { state: 'needs_trust', files, progress: null });
    this.pump();
  }

  private install(id: string, files: ModelItem['files']): void {
    this.set(id, { state: 'installing', files, progress: null });
    this.core.scheduler.setTimeout(() => this.set(id, { state: 'installed' }), ENGINE_STEP_MS);
  }

  private view(): ModelsView {
    return {
      items: [...this.items.values()],
      embedder: this.embedder,
      reindex: this.reindex,
      parallel: PARALLEL,
    };
  }

  private runReindex(index: string): ReindexView {
    if (this.reindexTimer !== null) this.core.scheduler.clearTimeout(this.reindexTimer);
    const total = 1200;
    const step = (done: number) => {
      const finished = done >= total;
      this.reindex = {
        running: !finished,
        embedder: index,
        databases: 3,
        rebuilt: finished ? 3 : 1,
        embedded: done,
        done,
        total,
        failed: 0,
        cancelled: false,
        finished,
      };
      this.core.emit([{ type: 'ReindexStatus', status: this.reindex }]);
      this.reindexTimer = finished
        ? null
        : this.core.scheduler.setTimeout(() => step(done + total / STEPS), ENGINE_STEP_MS);
    };
    step(0);
    return this.reindex;
  }

  api(): EnginesApi {
    const core = this.core;
    return {
      list: () => core.reply(this.view()),
      download: (itemId) => {
        const item = this.get(itemId);
        if (!item.downloadable) {
          return Promise.reject(
            new Error(`„${item.name}” instaluje się ręcznie — zobacz opis pozycji.`),
          );
        }
        const idle: readonly ModelItemState[] = [
          'missing',
          'paused',
          'failed',
          'corrupt',
          'external',
        ];
        if (idle.includes(item.state)) {
          this.set(itemId, { state: 'queued', error: null });
          this.pump();
        }
        return core.reply(this.get(itemId));
      },
      cancel: (itemId) => {
        const timer = this.timers.get(itemId);
        if (timer !== undefined) core.scheduler.clearTimeout(timer);
        this.timers.delete(itemId);
        const item = this.get(itemId);
        if (item.state === 'downloading' || item.state === 'queued') {
          this.set(itemId, { state: item.progress ? 'paused' : 'missing' });
          this.pump();
        }
        return core.reply(this.get(itemId));
      },
      verify: (itemId) => core.reply(this.get(itemId)),
      remove: (itemId) => {
        if (this.get(itemId).active) {
          return Promise.reject(
            new Error('To aktywny model wyszukiwania — najpierw przełącz wyszukiwanie na inny.'),
          );
        }
        const seed = SEEDS.find((s) => s[0] === itemId);
        if (!seed)
          return Promise.reject(new Error(`Nie znaleziono: pozycja katalogu „${itemId}”.`));
        const fresh = seedItem(seed);
        this.items.set(itemId, fresh);
        core.emit([{ type: 'ModelChanged', item: fresh }]);
        return core.reply(fresh);
      },
      trustHash: (itemId, hashes: TrustedHashes) => {
        const item = this.get(itemId);
        if (item.state !== 'needs_trust')
          return Promise.reject(new Error('Ta pozycja nie czeka na zgodę.'));
        const bad = item.files.find((f) => !f.pinned_sha256 && hashes[f.name] !== f.sha256);
        if (bad) {
          return Promise.reject(
            new Error(
              `SHA-256 pliku ${bad.name} różni się od policzonego przy pobraniu — zgoda odrzucona.`,
            ),
          );
        }
        this.install(itemId, item.files);
        return core.reply(this.get(itemId));
      },
      activateEmbedder: (model) => {
        if (model !== LEXICAL && this.items.get(model)?.state !== 'installed') {
          return Promise.reject(
            new Error(
              `Model „${model}” nie jest zainstalowany — pobierz go w Ustawieniach → Modele i silniki.`,
            ),
          );
        }
        const index = model === LEXICAL ? LEXICAL_INDEX : `${model}@3f2a9c41d0be/384`;
        this.embedder = {
          configured: model,
          active: model,
          index_id: index,
          dims: model === LEXICAL ? 256 : 384,
          error: null,
        };
        for (const item of this.items.values()) {
          if (item.kind === 'embed') this.set(item.id, { active: item.id === model });
        }
        this.runReindex(index);
        return core.reply(this.embedder);
      },
      reindexStart: () => core.reply(this.runReindex(this.embedder.index_id)),
      reindexCancel: () => {
        if (this.reindexTimer !== null) core.scheduler.clearTimeout(this.reindexTimer);
        this.reindexTimer = null;
        if (this.reindex.running) {
          this.reindex = { ...this.reindex, running: false, cancelled: true, finished: true };
          core.emit([{ type: 'ReindexStatus', status: this.reindex }]);
        }
        return core.reply(this.reindex);
      },
      reindexStatus: () => core.reply(this.reindex),
    };
  }
}
