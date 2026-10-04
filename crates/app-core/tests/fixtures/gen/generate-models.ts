// Fixture'y menedżera modeli (`models_*`, `embed_model_activate`, `search_reindex_*`, zdarzenia
// `ModelProgress`, `ModelChanged`, `ReindexStatus`) — część generatora `generate.ts` (atrapa +
// `TauriAlfaClient` z atrapą `invoke`): pobranie z przypiętym hashem, przerwanie i wznowienie,
// zgoda TOFU, weryfikacja, embedder wyszukiwania, przebudowa wektorów, usunięcie.
import { ENGINE_STEP_MS } from '../../../../../apps/desktop/ui/src/lib/api/fake/api-engines';
import type { VirtualScheduler } from '../../../../../apps/desktop/ui/src/lib/api/fake/fake-client';
import type { ModelsView } from '../../../../../apps/desktop/ui/src/lib/api/types-models';

type Both = (ns: string, method: string, ...args: unknown[]) => Promise<unknown>;

const E5 = 'multilingual-e5-small';

export async function runModels(
  both: Both,
  scheduler: VirtualScheduler,
  flush: () => Promise<void>,
): Promise<void> {
  const step = async (n: number) => {
    scheduler.advance(ENGINE_STEP_MS * n);
    await flush();
  };
  await both('engines', 'list');
  await both('engines', 'download', 'silero-vad');
  await both('engines', 'download', E5);
  await step(1);
  await both('engines', 'cancel', E5);
  await both('engines', 'download', E5);
  await step(8);
  const view = (await both('engines', 'list')) as ModelsView;
  const pending = view.items.find((i) => i.id === E5);
  const hashes: Record<string, string> = {};
  for (const f of pending?.files ?? []) {
    if (!f.pinned_sha256 && f.sha256) hashes[f.name] = f.sha256;
  }
  await both('engines', 'trustHash', E5, hashes);
  await step(2);
  await both('engines', 'verify', 'silero-vad');
  await both('engines', 'activateEmbedder', E5);
  await step(1);
  await both('engines', 'reindexStatus');
  await both('engines', 'reindexCancel');
  await both('engines', 'reindexStart');
  await step(6);
  await both('engines', 'activateEmbedder', 'lexical');
  await step(6);
  await both('engines', 'remove', 'silero-vad');
  await both('engines', 'list');
}
