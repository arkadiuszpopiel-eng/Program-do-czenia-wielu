// Stan jednej rozmowy: drzewo gałęzi (append-only) + adnotacje widoku + stosowanie strumienia.
// Zdarzenia przychodzą paczkami raz na klatkę (RafBatcher w AppState) — tu tylko mutacje stanu.
import type { AgentId } from '@alfa/ui-kit';
import type { AlfaClient } from '../api/client';
import type { ModelProfile, Turn, TurnAnnotation } from '../api/types';
import type { ChatStreamEvent } from '../api/types-system';
import { errorText } from '../api/command-error';
import { applyTurnEvent } from '../logic/apply-event';
import {
  addTurn,
  buildTree,
  emptyTree,
  leafId,
  projectPath,
  selectSibling,
  selectTurn,
  siblingInfo,
  streamingTurn,
  type SiblingInfo,
  type TurnTree,
} from '../logic/turn-tree';

export interface StreamHooks {
  /** Delta tekstu tury agentki w tej rozmowie (dla aria-live). */
  onText?(turnId: string, text: string): void;
  onStop?(turnId: string): void;
  /** Błąd akcji bez pola do zachowania (ponów, kontynuuj, stop, ocena, ukrycie) — toast okna. */
  onError?(error: unknown): void;
}

export class ConversationState {
  tree = $state<TurnTree>(emptyTree());
  annotations = $state<Record<string, TurnAnnotation>>({});
  loaded = $state(false);
  loadError = $state<string | null>(null);
  readonly path: Turn[] = $derived(projectPath(this.tree));
  readonly streaming: Turn | undefined = $derived(streamingTurn(this.path));

  constructor(
    private readonly client: AlfaClient,
    readonly sessionId: string,
    private readonly hooks: StreamHooks = {},
  ) {}

  async load(): Promise<void> {
    try {
      const snapshot = await this.client.turns.list(this.sessionId);
      this.tree = buildTree(snapshot.turns);
      this.annotations = { ...snapshot.annotations };
      this.loadError = null;
    } catch (error) {
      this.loadError = errorText(error);
    } finally {
      this.loaded = true;
    }
  }

  turn(id: string): Turn | undefined {
    return this.tree.turns[id];
  }

  siblings(turn: Turn): SiblingInfo {
    return siblingInfo(this.tree, turn);
  }

  selectVariant(turn: Turn, delta: number): void {
    selectSibling(this.tree, turn, delta);
  }

  apply(event: ChatStreamEvent): void {
    if (event.type === 'TurnAppended') {
      if (addTurn(this.tree, event.turn)) {
        const stored = this.tree.turns[event.turn.id];
        if (stored) selectTurn(this.tree, stored);
      }
      return;
    }
    const turn = this.tree.turns[event.turn_id];
    if (!turn) return;
    const result = applyTurnEvent(turn, event);
    if (result.text) this.hooks.onText?.(turn.id, result.text);
    if (result.stopped) this.hooks.onStop?.(turn.id);
  }

  /** Odrzuca przy błędzie: composer przywraca szkic (tekst użytkownika nie ginie). */
  async send(
    text: string,
    addressed: AgentId | null,
    profile: ModelProfile | null,
    attachments: readonly string[] = [],
  ): Promise<void> {
    await this.client.turns.send(this.sessionId, {
      parent_id: leafId(this.tree),
      text,
      addressed_to: addressed,
      profile,
      ...(attachments.length > 0 ? { attachments } : {}),
    });
  }

  regenerate(turn: Turn, profile: string | null = null): Promise<boolean> {
    return this.guard(() => this.client.turns.regenerate(this.sessionId, turn.id, profile));
  }

  /** Odrzuca przy błędzie: edytor tury zostaje otwarty z tekstem (MessageItem). */
  async editAndResend(turn: Turn, text: string): Promise<void> {
    await this.client.turns.editAndResend(this.sessionId, turn.id, text);
  }

  continueTurn(turn: Turn): Promise<boolean> {
    return this.guard(() => this.client.turns.continueTurn(this.sessionId, turn.id));
  }

  stop(): Promise<boolean> {
    return this.guard(() => this.client.turns.stop(this.sessionId));
  }

  /** Ocena działa od razu; odrzucona przez rdzeń wraca do poprzedniej. */
  rate(turn: Turn, rating: 'up' | 'down'): Promise<boolean> {
    const current = this.annotations[turn.id];
    const before = current?.rating ?? null;
    const next = before === rating ? null : rating;
    this.annotations[turn.id] = { hidden: current?.hidden ?? false, rating: next };
    return this.guard(
      () => this.client.turns.rate(turn.id, next),
      () => this.revert(turn.id, (a) => a.rating === next && { ...a, rating: before }),
    );
  }

  /** Ukrycie z widoku działa od razu; odrzucone przez rdzeń wraca. */
  setHidden(turn: Turn, hidden: boolean): Promise<boolean> {
    const before = this.annotations[turn.id]?.hidden ?? false;
    this.annotations[turn.id] = { rating: this.annotations[turn.id]?.rating ?? null, hidden };
    return this.guard(
      () => this.client.turns.setHidden(turn.id, hidden),
      () => this.revert(turn.id, (a) => a.hidden === hidden && { ...a, hidden: before }),
    );
  }

  /** Cofa adnotację, jeśli nie zmieniła jej w międzyczasie nowsza akcja (`false` — zostaw). */
  private revert(turnId: string, undo: (a: TurnAnnotation) => TurnAnnotation | false): void {
    const now = this.annotations[turnId];
    const restored = now && undo(now);
    if (restored) this.annotations[turnId] = restored;
  }

  /**
   * Akcja z własną obsługą błędu: cofnięcie zmiany optymistycznej, `onError` (toast okna) i wynik
   * `false` — przycisk z `void conv.stop()` nie gubi błędu. Bez `onError` błąd idzie dalej.
   */
  private async guard(action: () => Promise<unknown>, revert?: () => void): Promise<boolean> {
    try {
      await action();
      return true;
    } catch (error) {
      revert?.();
      if (!this.hooks.onError) throw error;
      this.hooks.onError(error);
      return false;
    }
  }
}
