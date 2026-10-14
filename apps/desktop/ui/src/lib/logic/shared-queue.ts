// Wspólna kolejka modeli wszystkich kont: pozycja to model konkretnego konta. W interfejsie
// pozycja ma klucz `<konto>/<model>` (identyfikator konta nie zawiera `/`, nazwa modelu — może),
// dzięki czemu ta sama lista kafelków obsługuje kolejkę konta i kolejkę wspólną.
import type { Account, ModelInfo, SharedQueueEntry } from '../api/types-hub';
import { MAX_QUEUE, type TaskClassId } from './model-queue';

/** Klucz pozycji w interfejsie. */
export const entryKey = (entry: SharedQueueEntry): string => `${entry.account_id}/${entry.model}`;

/** Pozycja z klucza (`null` — klucz bez konta). */
export function entryOf(key: string): SharedQueueEntry | null {
  const at = key.indexOf('/');
  if (at <= 0 || at === key.length - 1) return null;
  return { account_id: key.slice(0, at), model: key.slice(at + 1) };
}

/**
 * Opisy modeli pozycji dla kafelków (nazwa, przeznaczenie, kontekst). Konto pokazuje wiersz pod
 * nazwą. Model, którego konto nie ma na liście albo konto zniknęło, nie ma opisu — kafelek pokaże
 * wtedy, że go brakuje.
 */
export function entryModels(
  keys: readonly string[],
  accounts: readonly Account[],
): Map<string, ModelInfo> {
  const out = new Map<string, ModelInfo>();
  for (const key of keys) {
    const entry = entryOf(key);
    const account = entry && accounts.find((a) => a.id === entry.account_id);
    const model = account?.models.find((m) => m.id === entry?.model);
    if (model) out.set(key, model);
  }
  return out;
}

/**
 * Kolejka wspólna złożona z kolejek kont tej klasy: najpierw pozycje pierwszego konta, potem
 * następnego, bez powtórzeń i najwyżej {@link MAX_QUEUE} pozycji.
 */
export function fromAccountQueues(
  accounts: readonly Account[],
  taskClass: TaskClassId,
): SharedQueueEntry[] {
  const out: SharedQueueEntry[] = [];
  for (const account of accounts) {
    for (const model of account.queues?.[taskClass] ?? []) {
      if (out.length >= MAX_QUEUE) return out;
      if (!out.some((e) => e.account_id === account.id && e.model === model))
        out.push({ account_id: account.id, model });
    }
  }
  return out;
}
