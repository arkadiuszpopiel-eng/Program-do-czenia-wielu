// Interfejs menedżera modeli i silników (część `AlfaClient`; komendy `models_*`,
// `models_bundle*`, `embed_model_activate`, `search_reindex_*` w COMMANDS.md). Działania z gestu właściciela
// w Ustawieniach → „Modele i silniki” (i w onboardingu: lokalny model rozmowy).
import type {
  EmbedderView,
  ModelBundle,
  ModelItem,
  ModelsView,
  ReindexView,
  TrustedHashes,
} from './types-models';

/** Pobieranie (wznawiane, SHA-256 / zgoda TOFU), weryfikacja, usuwanie, embedder wyszukiwania. */
export interface EnginesApi {
  list(): Promise<ModelsView>;
  /** Start albo wznowienie w tle — postęp: `ModelProgress`, stan: `ModelChanged`. */
  download(itemId: string): Promise<ModelItem>;
  /** Przerywa pobieranie; plik częściowy zostaje do wznowienia. */
  cancel(itemId: string): Promise<ModelItem>;
  /** Ponowne SHA-256 zainstalowanych plików. */
  verify(itemId: string): Promise<ModelItem>;
  remove(itemId: string): Promise<ModelItem>;
  /** Jawna zgoda na pliki bez przypiętego hasha — hashe z karty (rdzeń porówna z policzonymi). */
  trustHash(itemId: string, hashes: TrustedHashes): Promise<ModelItem>;
  /** „Napraw”: usuwa pliki pozycji (z częściowymi pobraniami) i pobiera ją od nowa. */
  repair(itemId: string): Promise<ModelItem>;
  /** Pakiety od 6 (wzorcowy) do 1 (minimalny) dobrane do sprzętu tej maszyny. */
  bundles(): Promise<readonly ModelBundle[]>;
  /** Pobiera w tle brakujące i wstrzymane pozycje pakietu, uszkodzone naprawia. */
  bundleDownload(bundleId: string): Promise<ModelBundle>;
  /** Ponowne SHA-256 zainstalowanych pozycji pakietu. */
  bundleVerify(bundleId: string): Promise<ModelBundle>;
  /** `lexical` albo zainstalowany model embeddingów → przebudowa wektorów w tle. */
  activateEmbedder(model: string): Promise<EmbedderView>;
  reindexStart(): Promise<ReindexView>;
  reindexCancel(): Promise<ReindexView>;
  reindexStatus(): Promise<ReindexView>;
}
