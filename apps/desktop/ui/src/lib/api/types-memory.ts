// DTO Inspektora pamięci (F7, makieta 9; odpowiedniki `app-api/src/dto/memory.rs`): zakresy,
// wpisy z proweniencją, „dlaczego to pamiętam", edycja = nowa wersja, zapomnienie z podglądem
// kaskady, dziennik zmian z cofaniem, porządkowanie (Strażniczka pamięci).
// Identyfikator wpisu: `"<klucz zakresu>#<id>"` (np. `session:s-1#mem-1`, `global#mem-2`).
import type { TeachFile, TeachReport, TeachRequest } from './generated/ipc';
export type {
  ConsolidationReport,
  MemoryCascadeItem,
  MemoryEdit,
  MemoryExplanation,
  MemoryForgetPreview,
  MemoryForgetReport,
  MemoryForgetTarget,
  MemoryItem,
  MemoryJournalEntry,
  MemoryLayer,
  MemoryPage,
  MemoryQuery,
  MemoryScopeInfo,
  MemoryScopeKind,
  MemoryScopeRef,
  MemorySourceKind,
  MemorySourceLink,
  MemoryState,
  MemoryStatus,
  MemoryUndoResult,
  TeachFile,
  TeachReport,
  TeachRequest,
  TeachSource,
} from './generated/ipc';

/** „Naucz Alfę” (część `MemoryApi`). */
export interface MemoryTeachApi {
  /** Fakty z tekstu, pliku albo rozmowy (jako właściciel; od razu aktywne). */
  teach(request: TeachRequest): Promise<TeachReport>;
  /** Natywny dialog wyboru pliku i jego tekst (`null` — anulowano). */
  teachPickFile(): Promise<TeachFile | null>;
}
