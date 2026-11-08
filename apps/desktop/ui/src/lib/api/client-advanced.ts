// Interfejs strony Ustawienia → Zaawansowane (część `AlfaClient`; komendy `config_raw_*`).
import type { RawConfig } from './types-advanced';

/** Warstwa pliku konfiguracji. */
export type RawLayer = 'shared' | 'machine';

export interface AdvancedApi {
  raw(): Promise<RawConfig>;
  /** Kontrola składni: liczba kluczy albo błąd z numerem wiersza. */
  check(text: string): Promise<number>;
  /** Zapis z pełną walidacją rdzenia; odmowa przywraca poprzednią treść pliku. */
  save(layer: RawLayer, text: string): Promise<RawConfig>;
}
