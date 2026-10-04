// Menedżer modeli i silników (komendy `models_*`, `embed_model_activate`, `search_reindex_*`,
// zdarzenia `ModelProgress`, `ModelChanged`, `ReindexStatus`) — kształt 1:1
// z `crates/app-api/src/dto/models.rs` (pola snake_case). Rozmiary i liczniki w bajtach / sztukach.
import type { LocalizedText } from './types';

export type ModelItemKind =
  'llm' | 'stt' | 'tts' | 'vad' | 'wake' | 'speaker' | 'embed' | 'sidecar';

export type ModelItemState =
  | 'missing'
  | 'queued'
  | 'downloading'
  /** Przerwane — plik częściowy czeka na wznowienie. */
  | 'paused'
  /** Pobrane bez przypiętego SHA-256 — karta zgody z policzonym hashem. */
  | 'needs_trust'
  | 'installing'
  | 'installed'
  /** Pliki skopiowane ręcznie (bez rekordu weryfikacji). */
  | 'external'
  | 'corrupt'
  | 'failed';

export interface ModelFileView {
  readonly name: string;
  readonly url: string;
  readonly size_bytes: number;
  /** SHA-256 przypięty w katalogu (`null` — do przypięcia przez człowieka). */
  readonly pinned_sha256: string | null;
  /** SHA-256 policzony przy pobraniu (karta TOFU) albo zapisany przy instalacji. */
  readonly sha256: string | null;
}

export interface ModelProgressView {
  readonly file: string;
  readonly done: number;
  readonly total: number | null;
}

export interface ModelItem {
  readonly id: string;
  readonly kind: ModelItemKind;
  readonly name: string;
  readonly license: string;
  readonly source: string;
  readonly size_bytes: number;
  /** Katalog docelowy na tej maszynie. */
  readonly target: string;
  readonly files: readonly ModelFileView[];
  readonly state: ModelItemState;
  /** Wszystkie pliki z przypiętym SHA-256 (bez karty zgody). */
  readonly pinned: boolean;
  /** Adres, rozmiar i licencja potwierdzone przez człowieka (inaczej „do potwierdzenia”). */
  readonly confirmed: boolean;
  /** Do pobrania w aplikacji (inaczej — instalacja ręczna wg opisu). */
  readonly downloadable: boolean;
  readonly note: LocalizedText;
  readonly progress: ModelProgressView | null;
  readonly error: string | null;
  /** Embedder aktywny w wyszukiwaniu (tylko `embed`). */
  readonly active: boolean;
}

export interface EmbedderView {
  /** Wybór w ustawieniach: `lexical` albo identyfikator modelu. */
  readonly configured: string;
  /** Model w użyciu (`lexical`, gdy wybrany nie jest zainstalowany albo się nie załadował). */
  readonly active: string;
  /** Identyfikator w indeksie (`model_id/wymiar`). */
  readonly index_id: string;
  readonly dims: number;
  readonly error: string | null;
}

export interface ReindexView {
  readonly running: boolean;
  readonly embedder: string;
  readonly databases: number;
  readonly rebuilt: number;
  readonly embedded: number;
  /** Postęp bieżącej bazy. */
  readonly done: number;
  readonly total: number;
  readonly failed: number;
  readonly cancelled: boolean;
  readonly finished: boolean;
}

export interface ModelsView {
  readonly items: readonly ModelItem[];
  readonly embedder: EmbedderView;
  readonly reindex: ReindexView;
  /** Limit równoległych pobrań. */
  readonly parallel: number;
}

/** Zgoda TOFU: nazwa pliku → SHA-256 pokazany na karcie. */
export type TrustedHashes = Readonly<Record<string, string>>;
