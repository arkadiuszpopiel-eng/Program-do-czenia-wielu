// Interfejs aktualizacji i „O programie" (część `AlfaClient`; komendy `updates_*` w COMMANDS.md).
import type { AboutInfo, UpdatesView, WhatsNew } from './types-updates';

/** Aktualizacje: sprawdź → pobierz (z wznawianiem) → zweryfikuj → uruchom ponownie. */
export interface UpdatesApi {
  status(): Promise<UpdatesView>;
  /** „Sprawdź teraz" (manifest kanału przez HTTPS, bez telemetrii). */
  check(): Promise<UpdatesView>;
  /** Pobieranie w tle — postęp w zdarzeniach `UpdateStatus`. */
  download(): Promise<UpdatesView>;
  /** Przerywa pobieranie; wznowienie później od miejsca przerwania. */
  cancel(): Promise<UpdatesView>;
  /** Intencja: launcher uruchamia przygotowaną wersję po zamknięciu Alfy. */
  restart(): Promise<void>;
  /** „Przywróć poprzednią wersję" (działa od ponownego uruchomienia). */
  rollback(): Promise<UpdatesView>;
  about(): Promise<AboutInfo>;
  /** „Co nowego" — raz po aktualizacji (`null` — nic do pokazania). */
  whatsNew(): Promise<WhatsNew | null>;
  dismissWhatsNew(): Promise<void>;
}
