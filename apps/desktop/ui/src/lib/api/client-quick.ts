// Interfejs strony Ustawienia → Polecenia bez modelu (część `AlfaClient`; komendy `quick_commands_*`).
import type { CustomCommand, QuickCommandsView, QuickTest } from './types-quick';

export interface LocalCommandsApi {
  get(): Promise<QuickCommandsView>;
  /** Zapis całej listy własnych poleceń (rdzeń sprawdza nazwy, frazy, tryb i wyrażenia). */
  set(commands: readonly CustomCommand[]): Promise<QuickCommandsView>;
  /** Kto odpowie na wiadomość: własne polecenie, quiz, wbudowane albo model. */
  test(message: string): Promise<QuickTest>;
}
