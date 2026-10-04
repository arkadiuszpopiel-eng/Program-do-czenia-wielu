// Interfejs menedżera modeli i silników (część `AlfaClient`; komendy `models_*`,
// `embed_model_activate`, `search_reindex_*` w COMMANDS.md). Działania z gestu właściciela
// w Ustawieniach → „Modele i silniki” (i w onboardingu: lokalny model rozmowy).
import type {
  EmbedderView,
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
  /** `lexical` albo zainstalowany model embeddingów → przebudowa wektorów w tle. */
  activateEmbedder(model: string): Promise<EmbedderView>;
  reindexStart(): Promise<ReindexView>;
  reindexCancel(): Promise<ReindexView>;
  reindexStatus(): Promise<ReindexView>;
}
