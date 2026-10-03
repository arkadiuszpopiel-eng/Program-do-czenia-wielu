// Fixture'y komend aktualizacji i „O programie" (`updates_*`, zdarzenie `UpdateStatus`) — część
// generatora `generate.ts` (atrapa + `TauriAlfaClient` z atrapą `invoke`).
import { FAKE_STEP_MS } from '../../../../../apps/desktop/ui/src/lib/api/fake/api-updates';
import type { VirtualScheduler } from '../../../../../apps/desktop/ui/src/lib/api/fake/fake-client';

type Both = (ns: string, method: string, ...args: unknown[]) => Promise<unknown>;

export async function runUpdates(
  both: Both,
  scheduler: VirtualScheduler,
  flush: () => Promise<void>,
): Promise<void> {
  await both('updates', 'status');
  await both('updates', 'whatsNew');
  await both('updates', 'check');
  await both('updates', 'download');
  scheduler.advance(FAKE_STEP_MS);
  await flush();
  await both('updates', 'cancel');
  await both('updates', 'download');
  scheduler.advance(FAKE_STEP_MS * 10);
  await flush();
  await both('updates', 'restart');
  await both('updates', 'whatsNew');
  await both('updates', 'dismissWhatsNew');
  await both('updates', 'rollback');
  await both('updates', 'about');
  await flush();
}
