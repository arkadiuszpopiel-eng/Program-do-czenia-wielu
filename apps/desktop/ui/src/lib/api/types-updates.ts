// Aktualizacje i „O programie" (komendy `updates_*`, zdarzenie `UpdateStatus`) — kształt 1:1
// z `crates/app-api/src/dto/updates.rs` (pola snake_case). Wersje jako tekst semver.
import type { LocalizedText } from './types';

export type UpdatePhase =
  | 'disabled'
  | 'idle'
  | 'checking'
  | 'up_to_date'
  | 'available'
  | 'downloading'
  | 'verifying'
  | 'installing'
  | 'ready'
  | 'failed';

export type UpdateChannel = 'stable' | 'beta';
export type UpdateMode = 'auto' | 'ask' | 'manual';

export interface UpdateRelease {
  readonly version: string;
  /** „Co nowego" z manifestu — pokazywane jako zwykły tekst. */
  readonly notes: string;
}

export interface UpdateProgress {
  readonly downloaded: number;
  readonly total: number | null;
  /** Pobieranie wznowione (HTTP Range). */
  readonly resumed: boolean;
}

export interface UpdatesView {
  readonly phase: UpdatePhase;
  readonly current: string;
  readonly channel: UpdateChannel;
  readonly mode: UpdateMode;
  readonly available: UpdateRelease | null;
  /** Postęp pobierania (także częściowy plik czekający na wznowienie). */
  readonly progress: UpdateProgress | null;
  /** Wersja aktywna od ponownego uruchomienia (aktualizacja albo przywrócenie). */
  readonly ready: string | null;
  /** Cel „Przywróć poprzednią wersję". */
  readonly previous: string | null;
  readonly last_check: string | null;
  readonly error: string | null;
  /** Dlaczego teraz nie można uruchomić ponownie (zadanie agentki, rozmowa głosowa). */
  readonly restart_blocked: LocalizedText | null;
}

export type LicenseSource = 'cargo' | 'npm';

export interface LicenseEntry {
  readonly name: string;
  readonly version: string;
  /** Wyrażenie SPDX. */
  readonly license: string;
  readonly source: LicenseSource;
}

export interface AboutInfo {
  readonly version: string;
  readonly channel: UpdateChannel;
  /** Data kompilacji wydania (`null` — build deweloperski). */
  readonly build_date: string | null;
  readonly commit: string | null;
  readonly target: string;
  /** Adres wydań i klucz minisign wbudowane w wydanie. */
  readonly updates_configured: boolean;
  readonly licenses_generated_at: string | null;
  readonly licenses: readonly LicenseEntry[];
}

export interface WhatsNew {
  readonly version: string;
  /** Notatki z podpisanej paczki — pokazywane jako zwykły tekst. */
  readonly notes: string;
}
