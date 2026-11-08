// Fixture'y komend serii 6 — Arena (`turns_arena_answer`, `turns_select_branch`), karty rozmów
// (`sessions_cards`), ustawienia dla firm (`accounts_set_enterprise`), „Naucz Alfę”
// (`memory_teach`, `memory_teach_pick_file` — atrapa zamiast systemowego okna) i kanwa
// (`canvas_render`), cennik modeli (`pricing_get`, `pricing_apply`). Część generatora `generate.ts`, wywoływana na końcu, żeby nie przesuwać
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
  // Cennik modeli (0.0.66): odczyt, podgląd i zapis aktualizacji.
  await both('costs', 'pricing');
  const update = [
    'checked = "2026-11-01"',
    '[[price]]',
    'provider = "openai"',
    'model = "gpt-6-luna"',
    'input = 0.08',
    'output = 0.4',
  ].join('\n');
  await both('costs', 'applyPricing', update, false);
  await both('costs', 'applyPricing', update, true);
  await both('sessions', 'projectsList');
  await both('sessions', 'projectsPickDir');
  await both('sessions', 'projectsSave', 'Nowy Projekt', 'C:\\Users\\Ty\\Dokumenty\\Projekt');
  await both('sessions', 'projectsRename', 'Nowy Projekt', 'Projekt Alfa');
  await both('sessions', 'projectsRemove', 'Projekt Alfa');
  // Pipeline'y modeli: biblioteka (wzorce wbudowane), zapis własnego i usunięcie.
  await both('sessions', 'pipelinesList');
  await both(
    'sessions',
    'pipelinesSave',
    {
      name: 'Szkic do UI',
      description: 'Styl → kod',
      builtin: false,
      stages: [
        { title: 'Styl', role: 'thinker', persona: null, model: 'openai:gpt-4o-mini', goal: '{{input}}' },
        { title: 'Kod', role: 'coder', persona: 'delta', model: null, goal: 'Kod z: {{prev}}' },
      ],
    },
    null,
  );
  await both('sessions', 'pipelinesRemove', 'Szkic do UI');
}
