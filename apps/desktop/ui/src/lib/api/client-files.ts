// Interfejsy załączników composera, eksportu rozmowy i kopii zapasowych (część `AlfaClient`;
// komendy `attachments_*`, `sessions_export_conversation`, `backups_*` w COMMANDS.md).
import type { ExportResult } from './types-hub';
import type {
  AttachmentInfo,
  AttachmentsAdded,
  BackupCheck,
  BackupConfig,
  BackupView,
  ConversationFormat,
  DroppedFileHint,
} from './types-files';
import type { Unsubscribe } from './types-system';

/** Faza przeciągania plików nad oknem (tylko do podświetlenia strefy upuszczenia). */
export type DragPhase = 'enter' | 'leave' | 'drop';

/** Załączniki: kopie w katalogu sesji, przekazywane do tury identyfikatorami (`SendOptions`). */
export interface AttachmentsApi {
  /** Intencja: natywny dialog wyboru wielu plików. */
  pick(sessionId: string): Promise<AttachmentsAdded>;
  /**
   * Pliki z ostatniego upuszczenia na okno. W Tauri ścieżki zna wyłącznie powłoka (zdarzenie
   * systemowe) — `hints` nie są wysyłane; atrapa tworzy z nich załączniki.
   */
  addDropped(sessionId: string, hints: readonly DroppedFileHint[]): Promise<AttachmentsAdded>;
  /** Wklejenie plików albo obrazu ze schowka systemowego (czyta rdzeń, nie WebView). */
  paste(sessionId: string): Promise<AttachmentsAdded>;
  list(sessionId: string): Promise<readonly AttachmentInfo[]>;
  remove(sessionId: string, attachmentId: string): Promise<readonly AttachmentInfo[]>;
  /** Przeciąganie plików nad oknem (Tauri: zdarzenia `tauri://drag-*`); atrapa — bez zdarzeń. */
  watchDrag(handler: (phase: DragPhase) => void): Unsubscribe;
  /** Podgląd pliku przez protokół zasobów (`asset:`); `null` — brak (atrapa). */
  previewUrl(path: string): string | null;
}

export interface ConversationExportApi {
  /** Intencja: natywny dialog zapisu — aktywna gałąź (albo jedna wiadomość) do `.md` / `.html`. */
  exportConversation(
    sessionId: string,
    format: ConversationFormat,
    turnId: string | null,
  ): Promise<ExportResult>;
}

/** Kopie zapasowe `.alfa` z harmonogramem i rotacją (sekrety nigdy w paczce). */
export interface BackupsApi {
  status(): Promise<BackupView>;
  /** Odstęp, rotacja, zakres; katalogu nie zmienia (tylko `chooseDir`). */
  configure(config: BackupConfig): Promise<BackupView>;
  /** Intencja: natywny dialog wyboru katalogu. */
  chooseDir(): Promise<BackupView>;
  /** Hasło kopii w Credential Managerze (`null` — usuń; kopie bez szyfrowania). */
  setPassword(password: string | null): Promise<BackupView>;
  runNow(): Promise<BackupView>;
  /** Test przywracania wybranej kopii z katalogu (bez zapisu). */
  verify(file: string): Promise<BackupCheck>;
  /** „Przywróć…”: kopia z listy → jednorazowy uchwyt (15 s) dla `transfer.inspect`. */
  restore(file: string): Promise<string>;
}
