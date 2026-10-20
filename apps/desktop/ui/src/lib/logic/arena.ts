// Arena (P4): ta sama wiadomość do dwóch modeli obok siebie. Odpowiedzi są zwykłymi turami
// agentki w nowych gałęziach pod wiadomością użytkownika (append-only) — tu tylko czyste reguły:
// cel porównania, domyślna para modeli, kolejność uruchamiania (rdzeń prowadzi jedną generację
// na sesję, więc druga odpowiedź startuje po pierwszej), miary i porównanie wyników.
import type { Money, Turn, TurnStatus } from '../api/types';
import type { RoutingModel } from '../api/types-hub';
import { lastUserTurn } from './turn-tree';

/** Liczba porównywanych modeli. */
export const ARENA_SIZE = 2;

/** Stanowisko areny: model i jego odpowiedź (tura) albo błąd startu. */
export interface ArenaSlot {
  /** `dostawca:model`. */
  readonly model: string;
  turnId: string | null;
  error: string | null;
}

export type ArenaStep =
  | { readonly kind: 'start'; readonly index: number }
  | { readonly kind: 'wait' }
  | { readonly kind: 'done' };

/**
 * Wiadomość użytkownika, której dotyczy porównanie: wskazana tura (odpowiedź agentki → jej
 * rodzic) albo ostatnia wiadomość użytkownika widocznej ścieżki.
 */
export function arenaTarget(
  path: readonly Turn[],
  turns: Readonly<Record<string, Turn>>,
  turnId: string | null,
): Turn | undefined {
  if (turnId === null) return lastUserTurn(path);
  const turn = turns[turnId];
  if (!turn || turn.author === 'user') return turn;
  const parent = turn.parent_id ? turns[turn.parent_id] : undefined;
  return parent?.author === 'user' ? parent : undefined;
}

/** Domyślna para: model lokalny i z chmury, gdy są oba; inaczej dwa pierwsze różne. */
export function defaultPair(models: readonly RoutingModel[]): [string, string] | null {
  const local = models.find((m) => m.local);
  const cloud = models.find((m) => !m.local);
  if (local && cloud) return [local.candidate, cloud.candidate];
  const [a, b] = models;
  return a && b ? [a.candidate, b.candidate] : null;
}

/** Etykieta modelu do listy wyboru: „Dostawca · model”. */
export function modelLabel(model: RoutingModel): string {
  return `${model.provider_name} · ${model.model}`;
}

/**
 * Następny krok: uruchom kolejne stanowisko, gdy poprzednie się skończyło (albo nie wystartowało),
 * czekaj na trwającą odpowiedź, koniec. `stopped` — użytkownik przerwał: nic nowego nie startuje.
 */
export function arenaStep(
  slots: readonly ArenaSlot[],
  statusOf: (turnId: string) => TurnStatus | undefined,
  stopped: boolean,
): ArenaStep {
  for (const [index, slot] of slots.entries()) {
    if (slot.error !== null) continue;
    if (slot.turnId === null) return stopped ? { kind: 'done' } : { kind: 'start', index };
    const status = statusOf(slot.turnId);
    // Brak tury w drzewie — zdarzenie `TurnAppended` jeszcze nie dotarło.
    if (status === undefined || status === 'streaming' || status === 'queued')
      return { kind: 'wait' };
  }
  return { kind: 'done' };
}

/** Miary odpowiedzi do porównania. */
export interface ArenaMetrics {
  readonly latencyMs: number;
  readonly inputTokens: number;
  readonly outputTokens: number;
  readonly cost: Money;
  readonly provider: string;
  readonly model: string;
  /** Odpowiedział inny model niż wybrany (zapas Routera: limit, prywatność, awaria). */
  readonly fallback: boolean;
}

export function arenaMetrics(turn: Turn | undefined, requested: string): ArenaMetrics | null {
  const usage = turn?.usage;
  if (!usage) return null;
  const wanted = requested.slice(requested.indexOf(':') + 1);
  return {
    latencyMs: usage.latency_ms,
    inputTokens: usage.input_tokens,
    outputTokens: usage.output_tokens,
    cost: usage.cost,
    provider: usage.provider,
    model: usage.model,
    fallback: usage.model !== wanted && usage.model !== requested,
  };
}

/** Która odpowiedź była szybsza i tańsza (`null` — remis albo brak danych). */
export function arenaBest(metrics: readonly (ArenaMetrics | null)[]): {
  readonly faster: number | null;
  readonly cheaper: number | null;
} {
  const [a, b] = metrics;
  if (!a || !b) return { faster: null, cheaper: null };
  const pick = (x: number, y: number) => (x === y ? null : x < y ? 0 : 1);
  const comparable = a.cost.currency === b.cost.currency;
  return {
    faster: pick(a.latencyMs, b.latencyMs),
    cheaper: comparable ? pick(a.cost.minor, b.cost.minor) : null,
  };
}
