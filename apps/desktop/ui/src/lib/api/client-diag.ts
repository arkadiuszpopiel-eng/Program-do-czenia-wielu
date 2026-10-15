// Interfejs diagnostyki komputera (część `AlfaClient`; komendy `diag_*` w COMMANDS.md). Tylko
// pomiar — nic nie jest naprawiane ani zmieniane w systemie.
import type { CheckRun, DiagView } from './types-diag';

export interface DiagApi {
  /** Ostatni zapis z oceną, ostatnie testy, dostępne testy, postęp trwających. */
  state(): Promise<DiagView>;
  /** Nowy zapis parametrów (kilka sekund). */
  snapshot(): Promise<DiagView>;
  /** Testy z kluczy (`integrity`, `cpu_single`, `cpu_multi`, `memory`, `network`, `disk:C:`). */
  run(tests: readonly string[]): Promise<CheckRun>;
  cancel(): Promise<void>;
  /** Ostatnie przebiegi (najnowsze pierwsze). */
  runs(limit: number): Promise<readonly CheckRun[]>;
  /** Raport Markdown w folderze `Diagnostyka`; zwraca ścieżkę pliku. */
  export(): Promise<string>;
}
