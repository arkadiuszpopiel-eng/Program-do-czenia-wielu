// Logika strony „Modele lokalne i silniki" (bez Svelte — testowana w vitest): dostępne akcje pozycji,
// postęp, filtry rodzaju i stanu, hashe do zgody TOFU, postęp przebudowy wektorów.
import type {
  EmbedderView,
  ModelItem,
  ModelItemKind,
  ModelItemState,
  ReindexView,
  TrustedHashes,
} from '../api/types-models';

export type KindFilter = 'all' | ModelItemKind;
export type StateFilter = 'all' | 'installed' | 'available' | 'attention';

export const KIND_FILTERS: readonly KindFilter[] = [
  'all',
  'llm',
  'stt',
  'tts',
  'vad',
  'wake',
  'speaker',
  'embed',
  'sidecar',
];
export const STATE_FILTERS: readonly StateFilter[] = ['all', 'installed', 'available', 'attention'];

/** Embedder „bez modelu" (leksykalny). */
export const LEXICAL = 'lexical';

const BUSY: readonly ModelItemState[] = ['queued', 'downloading', 'installing'];
const ATTENTION: readonly ModelItemState[] = ['needs_trust', 'corrupt', 'failed', 'paused'];

/** Grupa stanu dla filtra. */
export function stateGroup(state: ModelItemState): Exclude<StateFilter, 'all'> {
  if (state === 'installed' || state === 'external') return 'installed';
  if (ATTENTION.includes(state)) return 'attention';
  return 'available';
}

export function filterItems(
  items: readonly ModelItem[],
  kind: KindFilter,
  state: StateFilter,
): readonly ModelItem[] {
  return items.filter(
    (i) =>
      (kind === 'all' || i.kind === kind) && (state === 'all' || stateGroup(i.state) === state),
  );
}

export interface ItemActions {
  readonly download: boolean;
  readonly resume: boolean;
  readonly cancel: boolean;
  readonly verify: boolean;
  readonly remove: boolean;
  readonly trust: boolean;
  /** „Używaj do wyszukiwania" (zainstalowany, nieaktywny embedder). */
  readonly activate: boolean;
  /** „Napraw": usunięcie plików (z częściowymi) i pobranie od nowa. */
  readonly repair: boolean;
}

export function itemActions(item: ModelItem): ItemActions {
  const s = item.state;
  const busy = BUSY.includes(s);
  const present = s === 'installed' || s === 'external' || s === 'corrupt';
  const partial = item.progress !== null && (s === 'paused' || s === 'failed');
  return {
    download:
      item.downloadable && (s === 'missing' || s === 'corrupt' || (s === 'failed' && !partial)),
    resume: item.downloadable && partial,
    cancel: s === 'queued' || s === 'downloading',
    verify: present,
    remove: item.downloadable && !busy && !item.active && s !== 'missing',
    trust: s === 'needs_trust',
    activate: item.kind === 'embed' && s === 'installed' && !item.active,
    repair: item.downloadable && !item.active && (present || s === 'failed' || partial),
  };
}

/** Procent pobrania (0–100) albo `null`, gdy rozmiar nieznany. */
export function progressPercent(item: ModelItem): number | null {
  const p = item.progress;
  if (!p || !p.total) return null;
  return Math.max(0, Math.min(100, Math.floor((p.done / p.total) * 100)));
}

/** Hashe z karty zgody (pliki bez przypiętego hasha, z hashem policzonym przy pobraniu). */
export function trustHashes(item: ModelItem): TrustedHashes {
  const out: Record<string, string> = {};
  for (const f of item.files) {
    if (!f.pinned_sha256 && f.sha256) out[f.name] = f.sha256;
  }
  return out;
}

/** Hash podzielony na grupy po 8 znaków (łatwiej porównać z `sha256sum`). */
export function groupedHash(hash: string): string {
  return hash.match(/.{1,8}/g)?.join(' ') ?? hash;
}

/** Procent przebudowy bieżącej bazy albo `null`. */
export function reindexPercent(view: ReindexView): number | null {
  if (!view.running || view.total <= 0) return null;
  return Math.max(0, Math.min(100, Math.floor((view.done / view.total) * 100)));
}

/** Opcje embeddera: leksykalny + zainstalowane modele embeddingów. */
export function embedderChoices(items: readonly ModelItem[]): readonly ModelItem[] {
  return items.filter((i) => i.kind === 'embed' && i.state === 'installed');
}

/** Wybrany model nie działa (niezainstalowany albo błąd ładowania) — wyszukiwanie leksykalne. */
export function embedderFallback(view: EmbedderView): boolean {
  return view.configured !== view.active;
}

/** Zastępuje pozycję na liście (zdarzenie `ModelChanged`). */
export function upsertItem(items: readonly ModelItem[], item: ModelItem): readonly ModelItem[] {
  return items.some((i) => i.id === item.id)
    ? items.map((i) => (i.id === item.id ? item : i))
    : [...items, item];
}

/** Rodzaje modeli, które mają pliki na dysku, ale nie mają silnika, który by je uruchomił. */
export type MissingEngine = 'llm' | 'stt' | 'tts';

const ENGINE_PREFIX: Record<MissingEngine, readonly string[]> = {
  llm: ['sidecar-llama'],
  stt: ['sidecar-whisper'],
  tts: ['sidecar-piper', 'sidecar-pocket'],
};

const onDisk = (item: ModelItem): boolean =>
  item.state === 'installed' || item.state === 'external';

/**
 * Model bez silnika jest bezużyteczny (test na laptopie 2026-10-07: pobrany Bielik bez
 * `llama-server`). Zwraca rodzaje, dla których jest model, a nie ma żadnego pasującego silnika.
 */
export function missingEngines(items: readonly ModelItem[]): MissingEngine[] {
  const engines = items.filter((i) => i.kind === 'sidecar' && onDisk(i)).map((i) => i.id);
  return (Object.keys(ENGINE_PREFIX) as MissingEngine[]).filter(
    (kind) =>
      items.some((i) => i.kind === kind && onDisk(i)) &&
      !engines.some((id) => ENGINE_PREFIX[kind].some((prefix) => id.startsWith(prefix))),
  );
}

/** Ocena „mieści się na karcie” dla modelu rozmowy. */
export type CardFit = 'fits' | 'partly' | 'cpu';

/** Zapas pamięci karty na kontekst i bufory modelu (MiB) — reguła z Delty AI: plik + 1,5 GB. */
export const CARD_HEADROOM_MB = 1536;
/** Poniżej tylu MiB pamięci karty model rozmowy liczy procesor. */
export const CARD_MIN_MB = 2000;

/**
 * Rezerwa z BIOS-u (MiB), od której karta zintegrowana ma własną pulę pamięci i liczy się jak karta
 * z tą pamięcią — ta sama wartość co `UMA_POOL_MIN_MB` w `device-profile-contract` (Rust).
 */
export const UMA_POOL_MIN_MB = 5500;

/** Karta z własną pulą pamięci: osobna albo zintegrowana z dużą rezerwą z BIOS-u. */
export function hasMemoryPool(gpu: {
  readonly vram_mb: number;
  readonly integrated: boolean;
}): boolean {
  return !gpu.integrated || gpu.vram_mb >= UMA_POOL_MIN_MB;
}

/**
 * Pamięć karty głównej (największa karta z własną pulą — osobna albo procesor z grafiką z rezerwą
 * ≥ 6 GB w BIOS-ie); `null` — nie ma takiej karty (sama grafika zintegrowana z małą rezerwą).
 */
export function mainCardMb(
  gpus: readonly { readonly vram_mb: number; readonly integrated: boolean }[],
): number | null {
  const sizes = gpus.filter(hasMemoryPool).map((g) => g.vram_mb);
  return sizes.length > 0 ? Math.max(...sizes) : null;
}

/**
 * Czy model o rozmiarze pliku `sizeBytes` zmieści się na karcie: cały („fits”), tylko część warstw
 * („partly” — reszta liczy się w pamięci RAM, wolniej) albo wcale („cpu”).
 */
export function cardFit(sizeBytes: number, cardMb: number | null): CardFit {
  if (cardMb === null || cardMb < CARD_MIN_MB) return 'cpu';
  const needMb = sizeBytes / (1024 * 1024) + CARD_HEADROOM_MB;
  return needMb <= cardMb ? 'fits' : 'partly';
}
