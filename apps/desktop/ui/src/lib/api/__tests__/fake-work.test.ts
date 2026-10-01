import { describe, expect, it } from 'vitest';
import { FakeAlfaClient, VirtualScheduler } from '../fake/fake-client';
import { cronNext } from '../fake/cron';
import type { AlfaEvent } from '../types-system';

const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

function setup() {
  const scheduler = new VirtualScheduler();
  const client = new FakeAlfaClient({ scheduler });
  const events: AlfaEvent[] = [];
  client.subscribe((batch) => events.push(...batch));
  return { client, scheduler, events };
}

const ALL = {
  scopes: [],
  text: null,
  layers: [],
  states: [],
  trusted: null,
  pinned: null,
  offset: 0,
  limit: 50,
} as const;

describe('atrapa: Inspektor pamięci', () => {
  it('filtry, wyjaśnienie z wersjami i edycja = nowa wersja', async () => {
    const { client, events } = setup();
    const untrusted = await client.memory.inspect({ ...ALL, trusted: false });
    expect(untrusted.items.every((i) => !i.trusted)).toBe(true);
    const why = await client.memory.explain('session:s-q3#m2');
    expect(why.versions.map((v) => v.version)).toEqual([1, 2]);
    const edited = await client.memory.edit('session:s-q3#m2', {
      text: 'Raport Q3 do poniedziałku.',
      subject: null,
      confidence: null,
    });
    expect(edited.version).toBe(3);
    expect((await client.memory.explain('session:s-q3#m2')).item.state).toBe('superseded');
    await flush();
    expect(events.some((e) => e.type === 'MemoryChanged')).toBe(true);
  });

  it('zapomnienie: podgląd kaskady (wersje i wywiedzione) = to, co znika', async () => {
    const { client } = setup();
    const target = { target: 'entry', id: 'session:s-q3#m2' } as const;
    const preview = await client.memory.forgetPreview(target);
    expect(preview.remove.map((r) => r.reason).sort()).toEqual(['derived', 'target', 'version']);
    const report = await client.memory.forget(target);
    expect(report.removed + report.versions + report.derived).toBe(preview.remove.length);
    const left = await client.memory.inspect(ALL);
    expect(left.items.some((i) => preview.remove.some((r) => r.id === i.id))).toBe(false);
  });

  it('dziennik: cofnięcie edycji przywraca poprzednią wersję', async () => {
    const { client } = setup();
    await client.memory.edit('global#m5', { text: 'Nowa treść', subject: null, confidence: null });
    const [change] = await client.memory.journal('global');
    expect(change?.kind).toBe('edit');
    const undo = await client.memory.undo('global', change?.id ?? '');
    expect(undo.restored).toBe(1);
    expect((await client.memory.explain('global#m5')).item.state).toBe('active');
  });

  it('zatwierdzenie propozycji agentki i awans do szerszego zakresu', async () => {
    const { client } = setup();
    const approved = await client.memory.approve('agent:beta#m4');
    expect(approved.state).toBe('active');
    const copy = await client.memory.promote('project:p-x#m3', { kind: 'global', id: null });
    expect(copy.scope_key).toBe('global');
    expect(copy.derivation).toBe('promoted');
  });
});

describe('atrapa: zadania i wyzwalacze', () => {
  it('zadanie przechodzi przez stany na wirtualnym zegarze; ponowienie = nowe zadanie', async () => {
    const { client, scheduler, events } = setup();
    const task = await client.tasks.create({
      session_id: 's-q3',
      title: '',
      goal: 'Sprawdź pisownię raportu',
      agent: null,
      after: [],
      parent_id: null,
    });
    scheduler.runAll();
    await flush();
    const done = (await client.tasks.list()).find((t) => t.id === task.id);
    expect(done?.result).toBe('succeeded');
    expect(events.filter((e) => e.type === 'TaskUpdated').length).toBeGreaterThan(2);
    const retried = await client.tasks.retry(task.id);
    expect(retried.id).toContain('.retry');
    await expect(client.tasks.steer(task.id, 'x')).rejects.toThrow();
  });

  it('anulowanie obejmuje poddrzewo delegacji', async () => {
    const { client } = setup();
    const parent = await client.tasks.create({
      session_id: null,
      title: 'Rodzic',
      goal: 'Rodzic',
      agent: null,
      after: ['t-marze'],
      parent_id: null,
    });
    await client.tasks.create({
      session_id: null,
      title: 'Dziecko',
      goal: 'Dziecko',
      agent: null,
      after: ['t-marze'],
      parent_id: parent.id,
    });
    const cancelled = await client.tasks.cancel(parent.id);
    expect(cancelled.length).toBe(2);
  });

  it('wyzwalacz ręczny tworzy zadanie z pochodzeniem „trigger" i wpis w dzienniku', async () => {
    const { client } = setup();
    const run = await client.triggers.fireNow('porzadki');
    const task = (await client.tasks.list()).find((t) => t.id === run.task_id);
    expect(task?.origin).toBe('trigger');
    expect((await client.triggers.log('porzadki')).length).toBe(1);
    const file = (await client.triggers.list()).find((t) => t.id === 'pobrane');
    expect(file?.watch_unavailable).toBe(true);
  });

  it('podgląd cron: dni robocze o 8:00, błąd dla złego wyrażenia', async () => {
    const { client } = setup();
    const ok = await client.triggers.previewCron('0 8 * * 1-5');
    expect(ok.valid).toBe(true);
    expect(ok.next).toHaveLength(5);
    for (const at of ok.next) {
      const d = new Date(at);
      expect(d.getHours()).toBe(8);
      expect([1, 2, 3, 4, 5]).toContain(d.getDay());
    }
    const bad = await client.triggers.previewCron('99 * * *');
    expect(bad.valid).toBe(false);
    expect('error' in cronNext('* * * * 8', 0, 1)).toBe(true);
  });
});

describe('atrapa: Marszałek i mosty', () => {
  it('propozycja → zatwierdzenie zawęża politykę; cofnięcie ją przywraca', async () => {
    const { client } = setup();
    const p = await client.marshal.propose('Nie używaj mostów CLI', null);
    expect(p.status).toBe('pending');
    await client.marshal.approve(p.id);
    let state = await client.marshal.state();
    expect(state.effective).toContain('mosty CLI zabronione');
    await client.marshal.revoke('bez-mostow');
    state = await client.marshal.state();
    expect(state.effective).toContain('mosty CLI dozwolone');
    const drafts = await client.marshal.propose('', [{ id: 'x', then: [] }, { foo: 1 }]);
    expect(drafts.rules).toHaveLength(1);
    expect(drafts.rejected).toHaveLength(1);
  });

  it('karty zgodności: zabronionej nie da się włączyć, przypięcie tylko wykrytej wersji', async () => {
    const { client } = setup();
    const cards = await client.bridges.list(false);
    expect(cards.find((c) => c.status === 'forbidden')?.can_enable).toBe(false);
    await expect(client.bridges.setEnabled('agy-antigravity', true)).rejects.toThrow();
    await expect(client.bridges.pin('claude_code', '9.9.9')).rejects.toThrow();
    const pinned = await client.bridges.pin('claude_code', '2.1.3');
    expect(pinned.version_ok).toBe(true);
    const limited = await client.bridges.setSchedule('claude_code', 99);
    expect(limited.schedule_per_day).toBe(24);
    const login = await client.bridges.openLogin('claude_code');
    expect(login.command).toBe('claude /login');
  });
});
