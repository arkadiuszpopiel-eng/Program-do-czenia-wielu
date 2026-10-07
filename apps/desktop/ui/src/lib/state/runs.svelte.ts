// Stan przebiegów agentek w oknie: ostatni przebieg per sesja (wiadomość w trakcie zadania,
// podpowiedź composera), cofnięte kroki (karty „Cofnij", Replay) i toast „Cofnij" po cofalnej akcji.
import type { AgentRun, ToolStep } from '../api/types';

const ACTIVE: readonly AgentRun['state'][] = ['running', 'waiting_approval', 'paused'];

/** Czas toastu „Cofnij" po cofalnej akcji agentki (PLAN §14.8). */
export const UNDO_TOAST_MS = 8_000;

export class RunsState {
  /** Ostatni przebieg per sesja. */
  bySession = $state<Record<string, AgentRun>>({});
  /** Cofnięte kroki (tokeny) — w tym oknie, do odświeżenia sesji. */
  undone = $state<Record<string, boolean>>({});
  /** Kroki, dla których pokazano już toast „Cofnij" (zwykły obiekt — bez reaktywności). */
  private readonly toasted: Record<string, true> = {};

  update(run: AgentRun): void {
    // Podprzebiegi (delegacja, Krytyczka, umiejętność) widać w Replay; „ostatni przebieg sesji"
    // (steering z composera) to zawsze przebieg główny.
    if (run.parent_id) return;
    this.bySession[run.session_id] = run;
  }

  /** Trwający przebieg w sesji (wiadomość z composera trafia do niego jako steering). */
  active(sessionId: string | null): AgentRun | null {
    if (!sessionId) return null;
    const run = this.bySession[sessionId];
    return run && ACTIVE.includes(run.state) ? run : null;
  }

  isUndone(token: string | null | undefined, serverUndone = false): boolean {
    return serverUndone || (token ? this.undone[token] === true : false);
  }

  markUndone(token: string): void {
    this.undone[token] = true;
  }

  /** Czy pokazać toast „Cofnij" dla zakończonego kroku (raz na krok). */
  shouldToast(step: ToolStep): boolean {
    if (step.status !== 'done' || !step.undo_token || step.undone) return false;
    if (this.toasted[step.id]) return false;
    this.toasted[step.id] = true;
    return true;
  }
}
