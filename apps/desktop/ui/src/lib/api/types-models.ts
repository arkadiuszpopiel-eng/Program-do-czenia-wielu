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

/** Stan pakietu na tej maszynie (ze stanów jego pozycji). */
export type BundleState =
  | 'not_installed'
  /** Część pozycji zainstalowana albo pobieranie wstrzymane. */
  | 'partial'
  | 'installed'
  /** Pozycja uszkodzona albo z błędem — do naprawy. */
  | 'corrupt'
  | 'downloading'
  /** Pobrane pozycje bez przypiętego SHA-256 czekają na zgodę (karta TOFU). */
  | 'needs_trust';

/** Dopasowanie do sprzętu: pasuje / na styk (zadziała z kompromisem) / za słaby. */
export type BundleFitKind = 'fits' | 'tight' | 'too_weak';

export interface BundleFit {
  readonly kind: BundleFitKind;
  /** Uzasadnienie (`tight`, `too_weak`). */
  readonly reason: LocalizedText | null;
}

/** Progi w MB z tolerancją raportowania systemu; `text` — wartości nominalne do pokazania. */
export interface BundleRequirements {
  readonly min_ram_mb: number;
  /** Pamięć karty (CUDA albo Vulkan); `null` — karta niepotrzebna. */
  readonly min_vram_mb: number | null;
  readonly min_cpu_cores: number | null;
  /** Karta konieczna; inaczej karta i procesor to alternatywy. */
  readonly gpu_required: boolean;
  readonly text: LocalizedText;
}

/** Pozycja pakietu (wariant silnika dobrany dla tej maszyny). */
export interface BundleItemView {
  readonly id: string;
  readonly name: string;
  readonly kind: ModelItemKind;
  readonly state: ModelItemState;
  readonly size_bytes: number;
  readonly downloadable: boolean;
  /** Silnik zapasowy (CPU), gdy wersja na kartę nie wystartuje. */
  readonly fallback: boolean;
}

/** Uwaga o jakości: zalecenie albo metoda pomiaru według normy (bez deklaracji certyfikacji). */
export interface QualityNote {
  readonly aspect: LocalizedText;
  /** Norma albo metoda, np. `ITU-T P.800 / P.808`. */
  readonly standard: string;
  readonly text: LocalizedText;
}

/** Pakiet w skali ocen 1–6 (6 — wzorcowy, 1 — minimalny) dla tej maszyny. */
export interface ModelBundle {
  readonly id: string;
  readonly rating: number;
  readonly name: LocalizedText;
  readonly summary: LocalizedText;
  readonly requirements: BundleRequirements;
  readonly items: readonly BundleItemView[];
  readonly size_bytes: number;
  /** Co najwyżej tyle zostało do pobrania. */
  readonly missing_bytes: number;
  readonly installed: number;
  readonly total: number;
  readonly state: BundleState;
  readonly fit: BundleFit;
  /** Najwyższy pakiet pasujący do tej maszyny bez kompromisów. */
  readonly recommended: boolean;
  readonly quality: readonly QualityNote[];
}
