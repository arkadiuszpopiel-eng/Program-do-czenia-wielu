// Stan jednej rozmowy: drzewo gałęzi (append-only) + adnotacje widoku + stosowanie strumienia.
// Zdarzenia przychodzą paczkami raz na klatkę (RafBatcher w AppState) — tu tylko mutacje stanu.
import type { AgentId } from '@alfa/ui-kit';
import type { AlfaClient } from '../api/client';
import type { ModelProfile, Turn, TurnAnnotation } from '../api/types';
import type { ChatStreamEvent } from '../api/types-system';
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
      this.loadError = error instanceof Error ? error.message : String(error);
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

  async regenerate(turn: Turn, profile: string | null = null): Promise<void> {
    await this.client.turns.regenerate(this.sessionId, turn.id, profile);
  }

  async editAndResend(turn: Turn, text: string): Promise<void> {
    await this.client.turns.editAndResend(this.sessionId, turn.id, text);
  }

  async continueTurn(turn: Turn): Promise<void> {
    await this.client.turns.continueTurn(this.sessionId, turn.id);
  }

  async stop(): Promise<void> {
    await this.client.turns.stop(this.sessionId);
  }

  async rate(turn: Turn, rating: 'up' | 'down'): Promise<void> {
    const current = this.annotations[turn.id];
    const next = current?.rating === rating ? null : rating;
    this.annotations[turn.id] = { hidden: current?.hidden ?? false, rating: next };
    await this.client.turns.rate(turn.id, next);
  }

  async setHidden(turn: Turn, hidden: boolean): Promise<void> {
    this.annotations[turn.id] = { rating: this.annotations[turn.id]?.rating ?? null, hidden };
    await this.client.turns.setHidden(turn.id, hidden);
  }
}
