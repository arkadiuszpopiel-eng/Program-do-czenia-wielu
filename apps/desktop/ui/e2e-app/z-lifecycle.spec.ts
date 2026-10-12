// Cykle życia przez łącze na PRAWDZIWYM rdzeniu (docs/plan/24 §2.3): sesje, wyzwalacze, zadania,
// koszty, ustawienia — każde polecenie wywołane naprawdę, a odpowiedź sprawdzona ze schematem
// łącza. Plik ma przedrostek `z-`, żeby biegł po teście dymnym (ten zakłada pierwsze uruchomienie).
// Profil jest tymczasowy; test i tak sprząta po sobie to, co utworzył.
import type { Page } from '@playwright/test';
import { expect, test } from './alfa';
import { loadIpcSchema, schemaErrors } from './ipc';

const schema = loadIpcSchema();

type Reply =
  { ok: true; value: unknown } | { ok: false; error: { code?: string; message?: string } };

async function raw(page: Page, command: string, args: Record<string, unknown>): Promise<Reply> {
  return page.evaluate(
    async ([name, payload]) => {
      const ipc = (
        window as unknown as {
          __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
        }
      ).__TAURI_INTERNALS__;
      try {
        return { ok: true as const, value: await ipc.invoke(name, payload) };
      } catch (error) {
        return { ok: false as const, error: error as { code?: string; message?: string } };
      }
    },
    [command, args] as const,
  );
}

/** Wywołanie, które ma się udać; wynik zgodny ze schematem polecenia. */
async function call<T = unknown>(
  page: Page,
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  const spec = schema.commands[command];
  expect(spec, `polecenie ${command} jest w schemacie`).toBeTruthy();
  const reply = await raw(page, command, args);
  expect(reply.ok, `${command}: ${reply.ok ? '' : JSON.stringify(reply.error)}`).toBe(true);
  const value = reply.ok ? (reply.value ?? null) : null;
  if (spec?.result) {
    expect(schemaErrors(schema, spec.result, value as never), `${command}: schemat`).toEqual([]);
  }
  return value as T;
}

/** Wywołanie, które ma zostać odrzucone czytelnym błędem z kodem. */
async function refused(
  page: Page,
  command: string,
  args: Record<string, unknown>,
): Promise<string> {
  const reply = await raw(page, command, args);
  expect(reply.ok, `${command} powinno zostać odrzucone`).toBe(false);
  const error = reply.ok ? {} : reply.error;
  expect(typeof error.code, `${command}: błąd ma kod`).toBe('string');
  expect((error.message ?? '').length, `${command}: błąd ma opis`).toBeGreaterThan(3);
  return error.code ?? '';
}

/**
 * Wywołanie, którego wynik zależy od wyścigu z rdzeniem (zadanie bez modelu kończy się samo):
 * sukces albo czytelna odmowa z kodem — nigdy cisza ani błąd bez opisu.
 */
async function settled(page: Page, command: string, args: Record<string, unknown>): Promise<void> {
  const reply = await raw(page, command, args);
  if (reply.ok) return;
  expect(typeof reply.error.code, `${command}: błąd ma kod`).toBe('string');
  expect((reply.error.message ?? '').length, `${command}: błąd ma opis`).toBeGreaterThan(3);
}

interface Session {
  id: string;
  title: string;
  pinned: boolean;
  archived: boolean;
}

test('sesje: pełny cykl życia przez łącze', async ({ alfa }) => {
  const { main } = alfa;
  const created = await call<Session>(main, 'sessions_create', { template: 'empty' });
  const id = created.id;
  await call(main, 'sessions_rename', { sessionId: id, title: 'Próba cyklu życia' });
  await call(main, 'sessions_set_pinned', { sessionId: id, pinned: true });
  await call(main, 'sessions_set_project', { sessionId: id, project: 'Projekt próbny' });
  await call(main, 'sessions_save_draft', { sessionId: id, text: 'szkic próbny' });
  expect(await call<string>(main, 'sessions_get_draft', { sessionId: id })).toBe('szkic próbny');
  await call(main, 'sessions_mark_read', { sessionId: id });
  await call(main, 'app_set_active_session', { sessionId: id });
  let list = await call<Session[]>(main, 'sessions_list');
  const renamed = list.find((s) => s.id === id);
  expect(renamed?.title).toBe('Próba cyklu życia');
  expect(renamed?.pinned).toBe(true);
  const hits = await call<{ session_id: string }[]>(main, 'sessions_search', {
    query: 'Próba cyklu',
  });
  expect(hits.some((h) => h.session_id === id)).toBe(true);
  const workdir = await call<{ default_path: string }>(main, 'sessions_workdir', { sessionId: id });
  expect(workdir.default_path.length).toBeGreaterThan(3);
  // Widoki sesji bez treści odpowiadają pustymi listami, nie błędem.
  await call(main, 'turns_list', { sessionId: id });
  await call(main, 'files_list', { sessionId: id });
  await call(main, 'agents_list', { sessionId: id });
  await call(main, 'agents_runs', { sessionId: id });
  await call(main, 'timeline_list', {
    sessionId: id,
    filter: { kinds: [], min_level: 'info' },
  });
  await call(main, 'costs_summary', { sessionId: id });
  // Szablon z sesji, archiwum, usunięcie z cofnięciem.
  const copy = await call<Session>(main, 'sessions_duplicate_as_template', { sessionId: id });
  expect(copy.id).not.toBe(id);
  await call(main, 'sessions_set_archived', { sessionId: copy.id, archived: true });
  await call(main, 'sessions_set_archived', { sessionId: copy.id, archived: false });
  const ticket = await call<{ token: string }>(main, 'sessions_remove', { sessionId: copy.id });
  list = await call<Session[]>(main, 'sessions_list');
  expect(list.some((s) => s.id === copy.id)).toBe(false);
  await call(main, 'sessions_undo_remove', { token: ticket.token });
  list = await call<Session[]>(main, 'sessions_list');
  expect(list.some((s) => s.id === copy.id)).toBe(true);
  // Nieznana sesja to czytelny błąd, nie cisza.
  await refused(main, 'sessions_rename', { sessionId: 'nie-ma-takiej', title: 'x' });
  await call(main, 'sessions_remove', { sessionId: copy.id });
  await call(main, 'sessions_remove', { sessionId: id });
});

interface Trigger {
  id: string;
  enabled: boolean;
}

test('wyzwalacze i zadania: tworzenie, uruchomienie, sterowanie, sprzątanie', async ({ alfa }) => {
  const { main } = alfa;
  const good = await call<{ valid: boolean; next: string[] }>(main, 'triggers_preview_cron', {
    expr: '0 8 * * 1-5',
  });
  expect(good.valid).toBe(true);
  expect(good.next).toHaveLength(5);
  const bad = await call<{ valid: boolean; error: string | null }>(main, 'triggers_preview_cron', {
    expr: 'to nie cron',
  });
  expect(bad.valid).toBe(false);
  expect((bad.error ?? '').length).toBeGreaterThan(3);

  const draft = {
    name: 'Próba łącza',
    kind: { kind: 'manual' },
    title: 'Próba łącza',
    goal: 'Napisz jedno zdanie.',
    agent: 'alfa',
    bridge: null,
    respect_dnd: false,
  };
  const first = await call<Trigger>(main, 'triggers_create', { draft });
  // Druga o tej samej nazwie dostaje własny identyfikator (błąd naprawiony w 0.0.45).
  const second = await call<Trigger>(main, 'triggers_create', { draft });
  expect(second.id).not.toBe(first.id);
  await call(main, 'triggers_set_enabled', { triggerId: first.id, enabled: false });
  let triggers = await call<Trigger[]>(main, 'triggers_list');
  expect(triggers.find((t) => t.id === first.id)?.enabled).toBe(false);
  await call(main, 'triggers_set_enabled', { triggerId: first.id, enabled: true });
  const run = await call<{ outcome: string; task_id: string | null }>(main, 'triggers_fire_now', {
    triggerId: first.id,
  });
  expect(run.outcome).toBe('submitted');
  const log = await call<{ trigger_id: string }[]>(main, 'triggers_log', { triggerId: first.id });
  expect(log.length).toBeGreaterThan(0);
  await call(main, 'triggers_log', { triggerId: null });
  if (run.task_id) await settled(main, 'tasks_cancel', { taskId: run.task_id });
  await refused(main, 'triggers_create', { draft: { ...draft, name: '  ', goal: '' } });

  // Zadanie użytkownika: wstrzymanie, wznowienie, wiadomość, anulowanie, ponowienie.
  const session = await call<Session>(main, 'sessions_create', { template: 'empty' });
  const task = await call<{ id: string }>(main, 'tasks_create', {
    input: {
      session_id: session.id,
      title: 'Zadanie próbne',
      goal: 'Policz do trzech.',
      agent: 'beta',
      after: [],
      parent_id: null,
    },
  });
  // Bez modelu zadanie może skończyć się samo — sterowanie po zakończeniu jest odrzucane z kodem.
  for (const [command, args] of [
    ['tasks_pause', { taskId: task.id }],
    ['tasks_resume', { taskId: task.id }],
    ['tasks_steer', { taskId: task.id, text: 'Wystarczy do dwóch.' }],
  ] as const) {
    await settled(main, command, args);
  }
  await settled(main, 'tasks_cancel', { taskId: task.id });
  const again = await call<{ id: string }>(main, 'tasks_retry', { taskId: task.id });
  expect(again.id).not.toBe(task.id);
  await settled(main, 'tasks_cancel', { taskId: again.id });
  const tasks = await call<{ id: string }[]>(main, 'tasks_list');
  expect(tasks.some((t) => t.id === task.id)).toBe(true);
  await refused(main, 'tasks_create', {
    input: { session_id: null, title: '', goal: '', agent: null, after: [], parent_id: null },
  });

  await call(main, 'triggers_remove', { triggerId: first.id });
  await call(main, 'triggers_remove', { triggerId: second.id });
  triggers = await call<Trigger[]>(main, 'triggers_list');
  expect(triggers.some((t) => t.id === first.id || t.id === second.id)).toBe(false);
  await call(main, 'sessions_remove', { sessionId: session.id });
});

test('koszty i ustawienia: zmiana, odczyt, powrót do wartości domyślnej', async ({ alfa }) => {
  const { main } = alfa;
  interface Costs {
    limit: { enabled: boolean; monthly: { minor: number; currency: string } };
  }
  const before = await call<Costs>(main, 'costs_summary', { sessionId: null });
  await call(main, 'costs_set_monthly_limit', {
    enabled: true,
    monthly: { minor: 4321, currency: 'PLN' },
  });
  const changed = await call<Costs>(main, 'costs_summary', { sessionId: null });
  expect(changed.limit.enabled).toBe(true);
  expect(changed.limit.monthly.minor).toBe(4321);
  await call(main, 'costs_set_monthly_limit', {
    enabled: before.limit.enabled,
    monthly: before.limit.monthly,
  });

  const values = await call<Record<string, unknown>>(main, 'settings_values');
  const original = values['ui.theme'];
  await call(main, 'settings_set', { key: 'ui.theme', value: 'dark' });
  expect((await call<Record<string, unknown>>(main, 'settings_values'))['ui.theme']).toBe('dark');
  const fallback = await call(main, 'settings_reset', { key: 'ui.theme' });
  expect(typeof fallback).toBe('string');
  if (typeof original === 'string' && original !== fallback) {
    await call(main, 'settings_set', { key: 'ui.theme', value: original });
  }
  await refused(main, 'settings_set', { key: 'nie.ma.takiego.ustawienia', value: true });
});
