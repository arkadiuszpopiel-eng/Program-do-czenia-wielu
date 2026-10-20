// Fixture'y komend serii 6 — Arena (`turns_arena_answer`, `turns_select_branch`), karty rozmów
// (`sessions_cards`), ustawienia dla firm (`accounts_set_enterprise`), „Naucz Alfę”
// (`memory_teach`, `memory_teach_pick_file` — atrapa zamiast systemowego okna) i kanwa
// (`canvas_render`). Część generatora `generate.ts`, wywoływana na końcu, żeby nie przesuwać
// identyfikatorów atrapy w pozostałych fixture'ach.
import type { VirtualScheduler } from '../../../../../apps/desktop/ui/src/lib/api/fake/fake-client';

type Both = (ns: string, method: string, ...args: unknown[]) => Promise<unknown>;

export async function runSeries6(
  both: Both,
  scheduler: VirtualScheduler,
  flush: () => Promise<void>,
): Promise<void> {
  const answer = (await both('turns', 'arenaAnswer', 's-q3', 'q3', 'openai:gpt-4o-mini')) as string;
  scheduler.runAll();
  await flush();
  await both('turns', 'selectBranch', 's-q3', answer);
  await both('sessions', 'cards', ['s-q3', 's-trip', 's-nieznana']);
  await both('accounts', 'setEnterprise', 'acc-openai', {
    organization: 'org-alfa',
    project: 'proj_raporty',
  });
  await both('memory', 'teachPickFile');
  await both('memory', 'teach', {
    session_id: 's-q3',
    source: { kind: 'text', text: 'Raport Q3 wysyłamy do 10. dnia miesiąca.\n\nZarząd czyta skrót.' },
    scope: 'session',
    title: null,
  });
  await both('memory', 'teach', {
    session_id: 's-q3',
    source: { kind: 'conversation' },
    scope: 'project',
    title: 'Raport Q3',
  });
  await both('conversation', 'renderArtifact', 'svg', '<svg xmlns="http://www.w3.org/2000/svg"/>', 'pl');
}
