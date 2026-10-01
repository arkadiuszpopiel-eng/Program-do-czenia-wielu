import { beforeAll, describe, expect, it } from 'vitest';
import type { AlfaEvent } from '../../api/types-system';
import { setupApp } from './helpers';

beforeAll(() => {
  globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) =>
    setTimeout(() => cb(0), 0) as unknown as number) as typeof requestAnimationFrame;
});

describe('Agentka z narzędziami: Replay, „Cofnij", steering, katalog roboczy', () => {
  it('zadanie na plikach → kroki Replay, toast „Cofnij" (8 s) i karta intencji terminala', async () => {
    const { app, client, advance } = setupApp();
    const seen: AlfaEvent[] = [];
    client.subscribe((batch) => seen.push(...batch));
    await app.start();
    expect(app.activeId).toBe('s-q3');
    const conv = app.conversation!;
    await conv.send('Delta, uporządkuj pliki w folderze Pobrane.', 'delta', null);
    await advance(0);
    expect(app.runs.active('s-q3')?.agent).toBe('delta');
    for (let i = 0; i < 40; i++) await advance(250);
    const steps = seen.filter((e) => e.type === 'AgentStep');
    expect(steps.length).toBeGreaterThanOrEqual(6);
    const runEvents = seen.filter((e) => e.type === 'AgentRunUpdated');
    expect(runEvents.at(-1)?.type === 'AgentRunUpdated' && runEvents.at(-1)?.run.state).toBe(
      'completed',
    );
    expect(app.runs.active('s-q3')).toBeNull();
    const undoToast = app.toasts.items.find((t) => t.actionLabel === 'Cofnij');
    expect(undoToast?.message).toBe('Delta: przeniesiono 14 plików');
    expect(undoToast?.timeoutMs).toBe(8_000);
    const turn = conv.path.at(-1)!;
    const terminal = turn.tools.find((s) => s.intent?.kind === 'open_in_terminal');
    expect(terminal?.intent?.command).toContain('Get-ChildItem');
    await client.agents.openTerminal(terminal!.id);
    const details = await client.agents.runs('s-q3');
    const last = details.at(-1)!;
    const undoable = last.steps.find((s) => s.undo_token);
    expect(undoable?.status).toBe('ok');
    await app.undoStep(undoable!.undo_token!, undoable!.title);
    expect(app.runs.isUndone(undoable!.undo_token)).toBe(true);
    const after = await client.agents.runs('s-q3');
    expect(after.at(-1)?.steps.find((s) => s.id === undoable!.id)?.undone).toBe(true);
  });

  it('wiadomość w trakcie zadania trafia do przebiegu (krok „steer")', async () => {
    const { app, client, advance } = setupApp();
    await app.start();
    await app.openSession('s-api');
    await app.conversation!.send('Delta, przenieś pliki do archiwum.', 'delta', null);
    await advance(0);
    await client.agents.steer('s-api', 'Pomiń pliki PDF');
    const runs = await client.agents.runs('s-api');
    expect(
      runs.at(-1)?.steps.some((s) => s.kind === 'steer' && s.input === 'Pomiń pliki PDF'),
    ).toBe(true);
    for (let i = 0; i < 40; i++) await advance(250);
    await expect(client.agents.steer('s-api', 'za późno')).rejects.toThrow(/nie wykonuje/);
  });

  it('Replay sesji z zatwierdzeniem: krok czeka, karta bez okna Brokera ma wyjaśnienie', async () => {
    const { client } = setupApp();
    const [seed] = await client.agents.runs('s-q3');
    expect(seed?.run.state).toBe('waiting_approval');
    expect(seed?.steps.at(-1)?.status).toBe('waiting_approval');
    const snapshot = await client.turns.list('s-q3');
    const approval = snapshot.turns.find((t) => t.approval)?.approval;
    expect(approval?.broker_window).toBe(true);
  });

  it('katalog roboczy: wybór w dialogu, katalog sesji, wyłączenie narzędzi', async () => {
    const { client } = setupApp();
    const start = await client.sessions.workdir('s-trip');
    expect(start.path).toBeNull();
    const picked = await client.sessions.chooseWorkdir('s-trip', 'dialog');
    expect(picked.path).toContain('Projekt');
    const def = await client.sessions.chooseWorkdir('s-trip', 'default');
    expect(def.path).toBe(def.default_path);
    expect((await client.sessions.chooseWorkdir('s-trip', 'none')).path).toBeNull();
  });
});

describe('Tryb głosowy', () => {
  it('rozmowa: pigułka z transkryptem częściowym, stan mikrofonu w UI, wyłączenie', async () => {
    const { app, client, advance } = setupApp();
    await app.start();
    await advance(0);
    expect(app.voice.status?.state).toBe('off');
    await client.voice.setMicEnabled(true);
    await advance(0);
    expect(app.voice.status?.state).toBe('active');
    await advance(400);
    await advance(400);
    expect(app.voice.pill?.speaker).toBe('user');
    expect(app.voice.pill?.partial).toBe('Co mam jutro');
    expect(app.micState).toBe('hearing');
    for (let i = 0; i < 4; i++) await advance(400);
    expect(app.voice.pill?.mic).toBe('speaking');
    await client.voice.setMicEnabled(false);
    await advance(0);
    expect(app.voice.status?.state).toBe('off');
    expect(app.micState).toBe('off');
  });

  it('bez mikrofonu: stan „niedostępny" z powodem, włączenie odrzucone', async () => {
    const { app, client, advance } = setupApp({ scenario: 'no-mic' });
    await app.start();
    await advance(0);
    expect(app.voice.available).toBe(false);
    expect(app.voice.status?.reason?.pl).toMatch(/Głos niedostępny/);
    await expect(client.voice.setMicEnabled(true)).rejects.toThrow(/niedostępny/);
  });
});
