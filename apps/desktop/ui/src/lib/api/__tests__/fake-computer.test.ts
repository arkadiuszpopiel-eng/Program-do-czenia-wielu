import { describe, expect, it } from 'vitest';
import { FakeAlfaClient, VirtualScheduler } from '../fake/fake-client';
import type { AlfaEvent } from '../types-system';
import type { AgentDraft, TerminalFrame } from '../types-work';
import { emptyDraft, textToB64 } from '../../logic/work';

const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

function setup(scenario?: 'empty') {
  const scheduler = new VirtualScheduler();
  const client = new FakeAlfaClient({ scheduler, scenario });
  const events: AlfaEvent[] = [];
  client.subscribe((batch) => events.push(...batch));
  return { client, scheduler, events };
}

describe('atrapa: panel Ekran (computer use)', () => {
  it('stan: Delta steruje Notatnikiem; akcje bez wpisywanej treści; zrzut tylko komendą', async () => {
    const { client, events } = setup();
    const status = await client.gui.status();
    expect(status.control?.agent).toBe('delta');
    expect(status.actions[0]?.summary).toBe('Wpisz tekst (24 znaków)');
    const shot = await client.gui.screenshot();
    expect(shot?.data_url.startsWith('data:image/svg+xml;base64,')).toBe(true);
    expect(shot?.info.masked).toBe(1);
    await client.gui.stop();
    await flush();
    const activity = events.filter((e) => e.type === 'GuiActivity');
    expect(activity).toHaveLength(1);
    expect(JSON.stringify(activity)).not.toContain('data:image');
  });

  it('„Zatrzymaj sterowanie" anuluje akcję w toku i przejmuje; „Oddaj" zwalnia', async () => {
    const { client } = setup();
    const stopped = await client.gui.stop();
    expect(stopped.taken_over).toBe(true);
    expect(stopped.control).toBeNull();
    expect(stopped.actions.find((a) => a.id === 3)?.status).toBe('cancelled');
    const released = await client.gui.release();
    expect(released.taken_over).toBe(false);
  });

  it('podgląd pulpitu: intencja → okno Brokera; nieznana sesja → błąd', async () => {
    const { client } = setup();
    expect((await client.gui.desktopGrant('s-q3', 'delta')).status).toBe('opened_broker');
    await expect(client.gui.desktopGrant('nie-ma', 'delta')).rejects.toThrow('Nieznana sesja');
  });
});

describe('atrapa: wbudowany terminal', () => {
  it('strumień tylko do `onFrame` (nie zdarzenia); wejście echo; zamknięcie → ramka exit', async () => {
    const { client, events } = setup();
    const frames: TerminalFrame[] = [];
    const term = await client.terminal.open('claude_login', 100, 30, null, (f) => frames.push(f));
    await flush();
    expect(frames[0]?.kind).toBe('output');
    await client.terminal.input(term.id, textToB64('sekret-123\r'));
    await flush();
    const text = frames
      .filter((f): f is Extract<TerminalFrame, { kind: 'output' }> => f.kind === 'output')
      .map((f) => atob(f.data_b64))
      .join('');
    expect(text).toContain('sekret-123');
    expect(JSON.stringify(events)).not.toContain('sekret');
    expect((await client.terminal.list()).map((s) => s.alive)).toEqual([true]);
    await client.terminal.close(term.id);
    await flush();
    expect(frames.at(-1)).toEqual({ kind: 'exit', code: 0 });
    await expect(client.terminal.input(term.id, textToB64('x'))).rejects.toThrow();
  });

  it('limit 3 terminali i walidacja rozmiaru', async () => {
    const { client } = setup();
    for (let i = 0; i < 3; i++) await client.terminal.open('shell', 80, 24, null, () => undefined);
    await expect(client.terminal.open('cmd', 80, 24, null, () => undefined)).rejects.toThrow('3');
    await expect(client.terminal.resize(1, 1, 1)).rejects.toThrow();
  });
});

describe('atrapa: umiejętności', () => {
  it('propozycja → przegląd (diff) → instalacja tylko z hashem → uruchomienie = zadanie', async () => {
    const { client, events } = setup();
    const review = await client.skills.review('porzadki-pobrane', '1.1.0');
    expect(review.previous_version).toBe('1.0.0');
    expect(review.diff.some((l) => l.kind === 'added')).toBe(true);
    await expect(client.skills.approve('porzadki-pobrane', '1.1.0', 'zly-hash')).rejects.toThrow(
      'Hash',
    );
    const installed = await client.skills.approve('porzadki-pobrane', '1.1.0', review.skill.hash);
    expect(installed.state).toBe('installed');
    const list = await client.skills.list();
    expect(list.find((s) => s.version === '1.0.0')?.state).toBe('superseded');
    await expect(client.skills.run('porzadki-pobrane', 's-q3', null, {})).rejects.toThrow('folder');
    const task = await client.skills.run('porzadki-pobrane', 's-q3', 'beta', { folder: 'C:\\x' });
    expect(task.title).toContain('Porządki');
    await flush();
    expect(events.some((e) => e.type === 'SkillsChanged')).toBe(true);
  });

  it('kwarantanna: zwolnienie wymaga hasha; odrzucenie i wyłączenie', async () => {
    const { client } = setup();
    const ext = (await client.skills.list()).find((s) => s.state === 'quarantined');
    expect(ext?.origin).toBe('external');
    await expect(
      client.skills.approve(ext?.id ?? '', ext?.version ?? '', ext?.hash ?? ''),
    ).rejects.toThrow();
    expect(
      (await client.skills.release(ext?.id ?? '', ext?.version ?? '', ext?.hash ?? '')).state,
    ).toBe('installed');
    expect((await client.skills.disable(ext?.id ?? '')).state).toBe('disabled');
    expect((await client.skills.reject('raport-tygodniowy', '1.0.0')).state).toBe('rejected');
  });
});

describe('atrapa: Kreator agentek', () => {
  const draft = (): AgentDraft => {
    const d = emptyDraft();
    if (!d.role) throw new Error('pusty szkic bez roli');
    return {
      ...d,
      name: 'Zofia',
      role: { ...d.role, name: 'Porządkowa', tools: ['fs'] },
      limits: { ...d.limits, fs_write: ['%USERPROFILE%\\Downloads\\**'] },
    };
  };

  it('rozmowa → szkic; podgląd z odmianą; zapis dopiero po teście na sucho tego hasha', async () => {
    const { client } = setup();
    const proposal = await client.builder.propose('Agentka o imieniu Zofia od pobranych plików');
    expect(proposal.draft.name).toBe('Zofia');
    const preview = await client.builder.preview(draft());
    expect(preview.forms).toHaveLength(7);
    expect(preview.forms.slice(1, 4)).toEqual(['Zofii', 'Zofii', 'Zofię']);
    await expect(client.builder.save(draft(), preview.hash)).rejects.toThrow('test na sucho');
    const dry = await client.builder.dryRun(draft());
    expect(dry.passed).toBe(true);
    expect(dry.hash).toBe(preview.hash);
    const saved = await client.builder.save(draft(), dry.hash);
    expect(saved.persona).toBe('zofia');
    expect((await client.builder.library()).map((a) => a.name)).toEqual(['Zofia']);
  });

  it('odmowy: zajęte imię i L4', async () => {
    const { client } = setup();
    await expect(client.builder.preview({ ...draft(), name: 'Delta' })).rejects.toThrow('zajęte');
    await expect(
      client.builder.preview({ ...draft(), limits: { ...draft().limits, autonomy: 'L4' } }),
    ).rejects.toThrow('L4');
  });
});

describe('atrapa: Zdrowie systemu', () => {
  it('propozycja naprawy → zgoda → naprawa z „Cofnij" → cofnięcie; Jądro tylko przez Brokera', async () => {
    const { client, events } = setup();
    const view = await client.health.report();
    expect(view.overall).toBe('degraded');
    expect(view.pending.map((p) => p.kernel)).toEqual([false, true]);
    await expect(client.health.approve(3)).rejects.toThrow('Brokera');
    const after = await client.health.approve(1);
    const repair = after.repaired[0];
    expect(repair?.undoable).toBe(true);
    const undone = await client.health.undo(repair?.id ?? 0);
    expect(undone.repaired[0]?.undoable).toBe(false);
    await flush();
    expect(events.filter((e) => e.type === 'HealthChanged').length).toBeGreaterThanOrEqual(2);
  });

  it('Ulepszacz: zatwierdzenie tylko z digestem; wycofanie; evale', async () => {
    const { client } = setup();
    const improver = await client.health.improver();
    expect(improver.idle_cycle).toBe(false);
    const [ready, failed] = improver.proposals;
    expect(failed?.can_approve).toBe(false);
    await expect(client.health.improverApprove(ready?.id ?? 0, 'inny')).rejects.toThrow('Diff');
    const deployed = await client.health.improverApprove(ready?.id ?? 0, ready?.digest ?? '');
    expect(deployed.proposals[0]?.stage).toBe('deployed');
    const rolled = await client.health.improverRollback(ready?.id ?? 0);
    expect(rolled.proposals[0]?.stage).toBe('rolled_back');
    const evals = await client.health.evals();
    expect(evals.suites.every((s) => s.integrity_ok)).toBe(true);
    expect((await client.health.evalsVerify('F4-agents')).id).toBe('F4-agents');
  });

  it('scenariusz pusty: brak propozycji i incydentów', async () => {
    const { client } = setup('empty');
    const view = await client.health.report();
    expect(view.overall).toBe('ok');
    expect(view.pending).toHaveLength(0);
    expect((await client.gui.status()).control).toBeNull();
    expect(await client.skills.list()).toHaveLength(0);
  });
});
