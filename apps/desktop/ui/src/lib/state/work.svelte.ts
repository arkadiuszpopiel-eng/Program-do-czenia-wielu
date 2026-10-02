// Stan F8 w oknie głównym: computer use (kto steruje — wskaźnik w pasku tytułu, panel „Ekran"),
// prośba o otwarcie wbudowanego terminala (dialog ładowany leniwie) i skrót „Zdrowia systemu".
import type { GuiStatus, HealthOverall, TerminalProfileId } from '../api/types-work';

export interface TerminalRequest {
  readonly profile: TerminalProfileId;
  /** Kolejny numer — ponowne kliknięcie tego samego profilu też otwiera dialog. */
  readonly seq: number;
}

export class WorkState {
  gui = $state<GuiStatus | null>(null);
  terminal = $state<TerminalRequest | null>(null);
  health = $state<{ overall: HealthOverall; pending: number } | null>(null);
  private seq = 0;

  /** Agentka steruje teraz ekranem (i właściciel nie przejął sterowania). */
  get controlling(): boolean {
    return Boolean(this.gui?.control) && !this.gui?.taken_over;
  }

  applyGui(status: GuiStatus): void {
    this.gui = status;
  }

  /** Gest użytkownika: otwórz terminal z profilem (powłoka albo logowanie mostu CLI). */
  openTerminal(profile: TerminalProfileId): void {
    this.terminal = { profile, seq: ++this.seq };
  }

  closeTerminal(): void {
    this.terminal = null;
  }
}
