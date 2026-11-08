// Załączniki composera (`attachments_*`), eksport rozmowy (`sessions_export_conversation`) i kopie
// zapasowe z harmonogramem (`backups_*`) — kształt 1:1 z `crates/app-api/src/dto/files.rs`
// (pola snake_case). Pliki nie przechodzą przez IPC: rdzeń kopiuje je do katalogu sesji `…\in`,
// a UI pokazuje podgląd przez protokół zasobów (`path`).
export type {
  AttachmentDelivery,
  AttachmentInfo,
  AttachmentKind,
  AttachmentRejectReason,
  AttachmentRejection,
  AttachmentsAdded,
  BackupCheck,
  BackupConfig,
  BackupEntry,
  BackupView,
  CanvasKind,
  ConversationFormat,
  TurnAttachment,
} from './generated/ipc';

/** Plik z przeciągnięcia w przeglądarce (tylko atrapa; w Tauri ścieżki zna wyłącznie powłoka). */
export interface DroppedFileHint {
  readonly name: string;
  readonly bytes: number;
  readonly mime: string;
}
