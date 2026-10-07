// Fixture'y komend F5–F7 (pamięć, zadania, wyzwalacze, Marszałek, mosty CLI) — część
// generatora `generate.ts` (ten sam mechanizm: atrapa + `TauriAlfaClient` z atrapą `invoke`).
import type { VirtualScheduler } from '../../../../../apps/desktop/ui/src/lib/api/fake/fake-client';

type Both = (ns: string, method: string, ...args: unknown[]) => Promise<unknown>;

const QUERY = {
  scopes: [],
  text: null,
  layers: [],
  states: [],
  trusted: null,
  pinned: null,
  offset: 0,
  limit: 50,
};

export async function runWork(
  both: Both,
  scheduler: VirtualScheduler,
  flush: () => Promise<void>,
): Promise<void> {
  await both('sessions', 'setProject', 's-trip', 'Wakacje');
  await both('sessions', 'setProject', 's-trip', null);

  await both('memory', 'status');
  await both('memory', 'scopes');
  await both('memory', 'inspect', QUERY);
  await both('memory', 'inspect', {
    ...QUERY,
    scopes: ['global'],
    text: 'dostaw',
    layers: ['semantic'],
    states: ['active'],
    trusted: false,
  });
  await both('memory', 'explain', 'session:s-q3#m2');
  await both('memory', 'edit', 'global#m5', {
    text: 'Komentarze w kodzie piszę po polsku, identyfikatory po angielsku.',
    subject: 'styl kodu',
    confidence: 0.95,
  });
  await both('memory', 'setPinned', 'project:p-x#m3', true);
  await both('memory', 'approve', 'agent:beta#m4');
  await both('memory', 'promote', 'project:p-x#m3', { kind: 'global', id: null });
  await both('memory', 'forgetPreview', { target: 'entry', id: 'session:s-q3#m2' });
  await both('memory', 'forgetPreview', { target: 'scope', scope: 'agent:beta' });
  await both('memory', 'forget', { target: 'entry', id: 'session:s-q3#m2' });
  const journal = (await both('memory', 'journal', 'global')) as { id: string }[];
  await both('memory', 'undo', 'global', journal[journal.length - 1]?.id ?? '');
  await both('memory', 'consolidateNow');
  await flush();

  await both('tasks', 'list');
  const task = (await both('tasks', 'create', {
    session_id: 's-q3',
    title: '',
    goal: 'Sprawdź pisownię raportu',
    agent: 'beta',
    after: [],
    parent_id: null,
  })) as { id: string };
  await both('tasks', 'create', {
    session_id: 's-q3',
    title: 'Wyślij raport',
    goal: 'Wyślij raport do zarządu',
    agent: null,
    after: [task.id],
    parent_id: null,
  });
  scheduler.advance(500);
  await flush();
  await both('tasks', 'steer', task.id, 'Pomiń tabele');
  await both('tasks', 'pause', task.id);
  await both('tasks', 'resume', task.id);
  scheduler.runAll();
  await flush();
  await both('tasks', 'retry', task.id);
  await both('tasks', 'cancel', 't-raport');

  await both('triggers', 'list');
  await both('triggers', 'previewCron', '0 8 * * 1-5');
  await both('triggers', 'previewCron', '61 * * * *');
  const created = (await both('triggers', 'create', {
    name: 'Kopia notatek',
    kind: { kind: 'interval', every_minutes: 90 },
    title: 'Kopia notatek',
    goal: 'Zrób kopię notatek ze spotkań',
    agent: 'beta',
    bridge: null,
    respect_dnd: true,
  })) as { id: string };
  await both('triggers', 'setEnabled', created.id, false);
  await both('triggers', 'fireNow', 'porzadki');
  await both('triggers', 'log', null);
  await both('triggers', 'log', 'porzadki');
  await both('triggers', 'remove', created.id);
  scheduler.runAll();
  await flush();

  await both('marshal', 'state');
  const proposal = (await both('marshal', 'propose', 'Nie używaj mostów CLI w nocy', null)) as {
    id: number;
  };
  await both('marshal', 'approve', proposal.id);
  const drafts = (await both('marshal', 'propose', 'Reguły z edytora', [
    { id: 'jedno-naraz', then: [{ effect: 'max_parallel', n: 1 }] },
    { description: 'bez id' },
  ])) as { id: number };
  await both('marshal', 'reject', drafts.id);
  await both('marshal', 'revoke', 'glos-pierwszy');
  await both('marshal', 'report');

  await both('bridges', 'list', false);
  await both('bridges', 'list', true);
  await both('bridges', 'setEnabled', 'claude-code-cli', false);
  await both('bridges', 'pin', 'claude_code', '2.1.3');
  await both('bridges', 'pin', 'claude_code', null);
  await both('bridges', 'setSchedule', 'claude_code', 2);
  await both('bridges', 'openLogin', 'claude_code');
  await flush();
}
