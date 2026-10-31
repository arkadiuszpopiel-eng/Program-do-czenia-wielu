// Polecenia łącza bez pokrycia do 0.0.80 (259 z 283 przećwiczonych), na PRAWDZIWYM rdzeniu.
// Projekty i potoki: pełny cykl (zapis, zmiana nazwy, usunięcie) wyłącznie w profilu instancji
// testowej, z powrotem do stanu wyjściowego. Kanwa, karty rozmów i „Naucz Alfę” — bez modelu
// (Markdown renderuje rdzeń, fakty wyciąga parser). Odmowy dla nieistniejących celów sprawdzone
// w kodzie rdzenia (2026-11-01): cennik bez ceny, nieznane konto, nieznana rozmowa — przed
// siecią i zapisem. Pomijane: okna systemowe (`projects_pick_dir`, `memory_teach_pick_file`,
// `skills_export/import`, `backups_choose_dir`), dźwięk (`builder_voice_preview`) i sieć
// (`updates_check`). Bez dźwięku, bez zrzutów ekranu, bez okien systemowych.
import { expect, test } from './alfa';
import { call, refused } from './ipc-call';

type Project = { name: string; workdir: string | null; session_count: number };
type Pipeline = { name: string; builtin: boolean; stages: unknown[] };

test.beforeEach(({ alfa }, info) => {
  alfa.log.phase = info.title;
});

test('projekty: zapis, zmiana nazwy i usunięcie w profilu testowym', async ({ alfa }) => {
  const { main } = alfa;
  const names = (list: Project[]) => list.map((p) => p.name);
  const before = await call<Project[]>(main, 'projects_list');
  let list = await call<Project[]>(main, 'projects_save', {
    name: 'Projekt e2e',
    workdir: null,
  });
  expect(names(list)).toContain('Projekt e2e');
  list = await call<Project[]>(main, 'projects_rename', {
    oldName: 'Projekt e2e',
    newName: 'Projekt e2e — nowa nazwa',
  });
  expect(names(list)).toContain('Projekt e2e — nowa nazwa');
  expect(names(list)).not.toContain('Projekt e2e');
  list = await call<Project[]>(main, 'projects_remove', {
    name: 'Projekt e2e — nowa nazwa',
  });
  expect(names(list)).toEqual(names(before));
  // Pusta nazwa bez katalogu — odmowa przed zapisem.
  await refused(main, 'projects_save', { name: '   ', workdir: null });
});

test('potoki: własny potok zapisany, odrzucony bez etapów, usunięty', async ({ alfa }) => {
  const { main } = alfa;
  const builtin = await call<Pipeline[]>(main, 'pipelines_list');
  expect(builtin.some((p) => p.builtin)).toBe(true);
  const pipeline = {
    name: 'Potok e2e',
    description: 'Szkic i poprawka (test prawdziwej aplikacji).',
    builtin: false,
    stages: [
      {
        title: 'Szkic',
        role: 'writer',
        persona: null,
        model: null,
        goal: 'Napisz: {{input}}',
      },
      {
        title: 'Poprawka',
        role: 'critic',
        persona: null,
        model: null,
        goal: 'Popraw: {{prev}}',
      },
    ],
  };
  await refused(main, 'pipelines_save', {
    pipeline: { ...pipeline, stages: [] },
    previousName: null,
  });
  let list = await call<Pipeline[]>(main, 'pipelines_save', {
    pipeline,
    previousName: null,
  });
  const saved = list.find((p) => p.name === 'Potok e2e');
  expect(saved?.builtin).toBe(false);
  expect(saved?.stages).toHaveLength(2);
  list = await call<Pipeline[]>(main, 'pipelines_remove', {
    name: 'Potok e2e',
  });
  expect(list.map((p) => p.name)).not.toContain('Potok e2e');
  expect(list).toHaveLength(builtin.length);
});

test('kanwa, karty rozmów i „Naucz Alfę” bez modelu', async ({ alfa }) => {
  const { main } = alfa;
  // Markdown z kanwy jak wiadomości: HTML ze składni, surowy HTML jako tekst.
  const html = await call<string>(main, 'canvas_render', {
    kind: 'markdown',
    source: '# Plan\n\n<script>alert(1)</script> gotowe',
    lang: 'pl',
  });
  expect(html).toContain('<h1>Plan</h1>');
  expect(html).not.toContain('<script');
  await refused(main, 'canvas_render', {
    kind: 'markdown',
    source: '   ',
    lang: 'pl',
  });

  const session = await call<{ id: string }>(main, 'sessions_create', {
    template: 'empty',
  });
  const cards = await call<{ session_id: string }[]>(main, 'sessions_cards', {
    sessionIds: [session.id],
  });
  expect(cards.map((c) => c.session_id)).toEqual([session.id]);
  expect(await call<unknown[]>(main, 'sessions_cards', { sessionIds: [] })).toEqual([]);

  const report = await call<{ facts: number; ids: string[] }>(main, 'memory_teach', {
    request: {
      session_id: session.id,
      source: {
        kind: 'text',
        text: 'Instancja testowa Alfy sprawdza naukę z tekstu. Ulubiony kolor testera to zielony.',
      },
      scope: 'session',
      title: null,
    },
  });
  expect(report.facts).toBeGreaterThan(0);
  expect(report.ids).toHaveLength(report.facts);
});

test('cennik, konto firmowe i gałęzie odpowiedzi: odmowa dla nieistniejących celów', async ({
  alfa,
}) => {
  const { main } = alfa;
  await refused(main, 'pricing_apply', {
    text: 'tu nie ma żadnej ceny',
    save: false,
  });
  await refused(main, 'accounts_set_enterprise', {
    accountId: 'brak-konta',
    enterprise: {
      organization: 'org-e2e',
      project: null,
      api_version: null,
      deployments: [],
    },
  });
  await refused(main, 'turns_select_branch', {
    sessionId: 'brak-sesji',
    turnId: 'brak',
  });
  await refused(main, 'turns_arena_answer', {
    sessionId: 'brak-sesji',
    turnId: 'brak',
    model: 'lokalny:brak',
  });
});
