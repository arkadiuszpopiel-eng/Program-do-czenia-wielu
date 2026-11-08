// Atrapa: wspólna kolejka modeli wszystkich kont (jak `accounts_shared_queues` /
// `accounts_set_shared_queue` w rdzeniu) — te same reguły i komunikaty odmowy.
import type { AlfaClient } from '../client';
import { CommandError } from '../command-error';
import type { SharedQueueEntry, SharedQueues } from '../types-hub';
import { MAX_QUEUE, TASK_CLASSES } from '../../logic/model-queue';
import type { FakeCore } from './core';

type SharedApi = Pick<AlfaClient['accounts'], 'sharedQueues' | 'setSharedQueue'>;

/** Wspólna kolejka atrapy (osobna na każdą instancję rdzenia atrapy). */
const STATE = new WeakMap<FakeCore, Record<string, readonly SharedQueueEntry[]>>();

export function sharedQueueApi(core: FakeCore): SharedApi {
  const queues = () => STATE.get(core) ?? {};
  const refuse = (code: 'invalid_input' | 'not_found', message: string) =>
    Promise.reject(new CommandError(code, message));
  return {
    sharedQueues: () => core.reply<SharedQueues>({ ...queues() }),
    setSharedQueue: (taskClass, entries) => {
      if (!TASK_CLASSES.some((c) => c === taskClass))
        return refuse('invalid_input', `Nieznana klasa zadań „${taskClass}”.`);
      if (entries.length > MAX_QUEUE)
        return refuse('invalid_input', `Wspólna kolejka może mieć najwyżej ${MAX_QUEUE} modeli.`);
      const clean: SharedQueueEntry[] = [];
      for (const entry of entries) {
        if (!core.accounts.some((a) => a.id === entry.account_id))
          return refuse('not_found', `Konto „${entry.account_id}” nie istnieje.`);
        const model = entry.model.trim();
        if (!model)
          return refuse('invalid_input', 'Nazwa modelu w kolejce jest pusta albo niepoprawna.');
        if (clean.some((e) => e.account_id === entry.account_id && e.model === model))
          return refuse('invalid_input', `Model „${model}” tego konta jest w kolejce dwa razy.`);
        clean.push({ account_id: entry.account_id, model });
      }
      const next: Record<string, readonly SharedQueueEntry[]> = { ...queues() };
      if (clean.length) next[taskClass] = clean;
      else delete next[taskClass];
      STATE.set(core, next);
      return core.reply<SharedQueues>({ ...next });
    },
  };
}
