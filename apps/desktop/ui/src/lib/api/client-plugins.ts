// Interfejs wtyczek Wasm (część `AlfaClient`; komendy `plugins_*` w COMMANDS.md). Metody zmieniające
// stan woła wyłącznie strona Ustawienia → „Wtyczki” z gestu właściciela (kanał zatwierdzenia UI).
import type { PluginInfo, PluginInspection, PluginsView } from './types-plugins';

/** Wtyczki: propozycja → karta (zdolności, limity, hash) → zatwierdzenie kliknięciem → narzędzia. */
export interface PluginsApi {
  list(): Promise<PluginsView>;
  /** Kontrola modułu (base64) bez instalacji. */
  inspect(wasmB64: string): Promise<PluginInspection>;
  /** Manifest (JSON) + moduł (base64) → propozycja czekająca na zatwierdzenie. */
  propose(manifest: unknown, wasmB64: string): Promise<PluginInfo>;
  /** Instalacja z hashem przejrzanej wersji (rdzeń odmówi, gdy treść się zmieniła). */
  approve(pluginId: string, version: string, reviewedHash: string): Promise<PluginInfo>;
  reject(pluginId: string, version: string): Promise<PluginInfo>;
  disable(pluginId: string): Promise<PluginInfo>;
  /** Ponowne włączenie = ponowne zatwierdzenie hashem. */
  enable(pluginId: string, reviewedHash: string): Promise<PluginInfo>;
  remove(pluginId: string): Promise<PluginsView>;
}
