// Załączniki composera (`attachments_*`), eksport rozmowy (`sessions_export_conversation`) i kopie
// zapasowe z harmonogramem (`backups_*`) — kształt 1:1 z `crates/app-api/src/dto/files.rs`
// (pola snake_case). Pliki nie przechodzą przez IPC: rdzeń kopiuje je do katalogu sesji `…\in`,
// a UI pokazuje podgląd przez protokół zasobów (`path`).
import type { Iso8601 } from './types';

export type AttachmentKind = 'image' | 'text' | 'document';

/** Co trafi do modelu: całość, tekst ucięty do limitu albo tylko nazwa, typ i rozmiar. */
export type AttachmentDelivery = 'full' | 'truncated' | 'metadata_only';

export interface AttachmentInfo {
  readonly id: string;
  readonly session_id: string;
  readonly name: string;
  readonly bytes: number;
  readonly mime: string;
  readonly kind: AttachmentKind;
  /** Kopia w katalogu sesji — podgląd przez `asset:` (nigdy bajty przez IPC). */
  readonly path: string;
  /** Szacunek tokenów wejścia modelu (heurystyka: znaki / 4, obraz ≈ 1600). */
  readonly tokens: number;
  readonly delivery: AttachmentDelivery;
}

export type AttachmentRejectReason =
  'too_large' | 'too_many' | 'total_too_large' | 'denied' | 'not_a_file' | 'unreadable' | 'empty';

export interface AttachmentRejection {
  readonly name: string;
  readonly reason: AttachmentRejectReason;
}

export interface AttachmentsAdded {
  readonly added: readonly AttachmentInfo[];
  readonly rejected: readonly AttachmentRejection[];
  /** Wszystkie przygotowane w sesji (po zmianie). */
  readonly staged: readonly AttachmentInfo[];
}

/** Plik z przeciągnięcia w przeglądarce (tylko atrapa; w Tauri ścieżki zna wyłącznie powłoka). */
export interface DroppedFileHint {
  readonly name: string;
  readonly bytes: number;
  readonly mime: string;
}

/** Załącznik wysłanej tury (artefakt sesji). */
export interface TurnAttachment {
  readonly name: string;
  readonly mime: string;
  readonly kind: AttachmentKind;
  readonly bytes: number | null;
  readonly artifact_id: string | null;
}

export type ConversationFormat = 'markdown' | 'html';

/** Ustawienia kopii (per maszyna). `dir` zmienia wyłącznie natywny dialog (`backups_choose_dir`). */
export interface BackupConfig {
  readonly enabled: boolean;
  readonly dir: string | null;
  readonly interval_hours: number;
  readonly keep: number;
  readonly include_artifacts: boolean;
  readonly include_logs: boolean;
  readonly skip_on_battery: boolean;
}

export interface BackupEntry {
  readonly file: string;
  readonly path: string;
  readonly created_at: Iso8601;
  readonly bytes: number;
}

export interface BackupView {
  readonly config: BackupConfig;
  /** Hasło kopii w Credential Managerze — kopie szyfrowane, z sesjami prywatnymi. */
  readonly password_set: boolean;
  /** Najnowsze pierwsze. */
  readonly entries: readonly BackupEntry[];
  readonly last_run: Iso8601 | null;
  readonly last_error: string | null;
  readonly next_due: Iso8601 | null;
  readonly running: boolean;
}

/** Test przywracania: otwarcie, sumy kontrolne, odszyfrowanie i dry-run (bez zapisu). */
export interface BackupCheck {
  readonly file: string;
  readonly ok: boolean;
  readonly encrypted: boolean;
  readonly created_at: Iso8601 | null;
  readonly app_version: string | null;
  readonly items: number;
  readonly sessions: number;
  readonly message: string | null;
}
