// Interfejs strony Ustawienia → Moduły (część `AlfaClient`; komendy `modules_*` w COMMANDS.md).
import type { ModulesView } from './types-modules';

export interface ModulesApi {
  list(): Promise<ModulesView>;
  /** Włącza / wyłącza moduł opcjonalny (zmiana po ponownym uruchomieniu). */
  setEnabled(id: string, enabled: boolean): Promise<ModulesView>;
}
