// Regresje z przeglądu logiki stanu interfejsu (2026-10-09, docs/STATUS.md).
import { beforeAll, describe, expect, it, vi } from 'vitest';
import type { AlfaEvent } from '../../api/types-system';
import type { AlfaClient } from '../../api/client';
import type { UpdatesApi } from '../../api/client-updates';
import { session, turn } from '../../api/fake/fixtures';
import { Debouncer } from '../background';
import { ConversationState } from '../conversation.svelte';
import { LayoutState } from '../layout.svelte';
import { SessionsState } from '../sessions.svelte';
import { UpdatesState } from '../updates.svelte';
import { setupApp } from './helpers';

beforeAll(() => {
  globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) =>
    setTimeout(() => cb(0), 0) as unknown as number) as typeof requestAnimationFrame;
});

const T = Date.UTC(2026, 9, 9, 12);

describe('panele bez aktywnej sesji', () => {
  it('przełączenie panelu jest potem widoczne', () => {
    const layout = new LayoutState();
    layout.width = 1600;
    const before = layout.current(null).right_open;
    layout.update(null, { right_open: !before });
    expect(layout.current(null).right_open).toBe(!before);
  });
});

describe('zamknięte karty', () => {
  it('usunięta sesja nie wraca przez „Przywróć zamkniętą kartę”', () => {
    const s = new SessionsState();
    s.list = [session('a', 'A', T), session('b', 'B', T - 1), session('c', 'C', T - 2)];
    s.close('a');
    s.close('b');
    s.remove('b');
    expect(s.reopen()).toBe('a');
    expect(s.reopen()).toBeUndefined();
  });

  it('usunięcie sesji czyści jej szkic', () => {
    const s = new SessionsState();
    s.list = [session('a', 'A', T)];
    s.drafts['a'] = 'niedokończone';
    s.remove('a');
    expect(s.drafts['a']).toBeUndefined();
  });
});

describe('zapisy w tle', () => {
  it('zamknięcie wykonuje zaległe zadania zamiast je porzucać', () => {
    vi.useFakeTimers();
    try {
      const done: string[] = [];
      const debouncer = new Debouncer(400);
      debouncer.run('draft:a', () => done.push('stary'));
      debouncer.run('draft:a', () => done.push('a'));
      debouncer.run('layout', () => done.push('layout'));
      debouncer.dispose();
      expect(done.sort()).toEqual(['a', 'layout']);
      vi.advanceTimersByTime(1000);
      expect(done).toHaveLength(2);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('stan aktualizacji', () => {
  const ready = { phase: 'ready', restart_blocked: null } as unknown as Awaited<
    ReturnType<UpdatesApi['status']>
  >;

  it('błąd „Co nowego” nie kasuje informacji o gotowej wersji', async () => {
    const updates = new UpdatesState();
    await updates.load({
      status: () => Promise.resolve(ready),
      whatsNew: () => Promise.reject(new Error('brak')),
    } as unknown as UpdatesApi);
    expect(updates.view?.phase).toBe('ready');
    expect(updates.whatsNew).toBeNull();
  });

  it('zdarzenie, które przyszło w trakcie pytania o stan, wygrywa ze starszą odpowiedzią', async () => {
    const updates = new UpdatesState();
    let answer: (v: typeof ready) => void = () => undefined;
    const loading = updates.load({
      status: () => new Promise<typeof ready>((resolve) => (answer = resolve)),
      whatsNew: () => Promise.resolve(null),
    } as unknown as UpdatesApi);
    updates.applyEvent(ready);
    answer({ ...ready, phase: 'idle' } as typeof ready);
    await loading;
    expect(updates.view?.phase).toBe('ready');
  });
});

describe('AppState — zdarzenia', () => {
  it('wadliwe zdarzenie nie gubi reszty paczki', async () => {
    const { app } = setupApp();
    await app.start();
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    try {
      const broken = { type: 'ModelChanged' } as unknown as AlfaEvent;
      const removed: AlfaEvent = { type: 'SessionRemoved', session_id: 's-trip' };
      app.applyBatch([broken, removed]);
      await Promise.resolve();
      expect(app.sessions.list.some((s) => s.id === 's-trip')).toBe(false);
      expect(warn).toHaveBeenCalled();
    } finally {
      warn.mockRestore();
    }
  });

  it('sesja usunięta poza tym oknem: aktywna rozmowa przełącza się na następną', async () => {
    const { app } = setupApp();
    await app.start();
    const active = app.activeId;
    expect(active).not.toBeNull();
    await app.sessionRemoved(active ?? '');
    expect(app.sessions.list.some((s) => s.id === active)).toBe(false);
    expect(app.activeId).not.toBe(active);
    expect(app.conversation?.sessionId).toBe(app.activeId);
  });
});

describe('rozmowa — zdarzenia w trakcie wczytywania', () => {
  it('koniec tury, który przyszedł przed migawką, nie zostawia jej w stanie „pisze”', async () => {
    const streaming = {
      ...turn('s:t2', 's', null, 'gama', 'Początek', T),
      status: 'streaming' as const,
    };
    let answer: (v: unknown) => void = () => undefined;
    const client = {
      turns: { list: () => new Promise((resolve) => (answer = resolve)) },
    } as unknown as AlfaClient;
    let stops = 0;
    const conv = new ConversationState(client, 's', { onStop: () => stops++ });
    const loading = conv.load();
    conv.apply({ type: 'Stop', session_id: 's', turn_id: streaming.id, reason: 'end' });
    answer({ turns: [streaming], annotations: {} });
    await loading;
    expect(conv.turn(streaming.id)?.status).toBe('complete');
    expect(conv.streaming).toBeUndefined();
    expect(stops).toBe(1);
  });
});

// Regresje z przeglądu zmian 0.0.1–0.0.5 (świeży kontekst, 2026-10-09).
describe('przegląd 0.0.5', () => {
  it('dwa równoległe wczytania rozmowy to jedno zapytanie i żadne zdarzenie nie ginie', async () => {
    const streaming = {
      ...turn('s:t2', 's', null, 'gama', 'Początek', T),
      status: 'streaming' as const,
    };
    const answers: ((v: unknown) => void)[] = [];
    const client = {
      turns: { list: () => new Promise((resolve) => answers.push(resolve)) },
    } as unknown as AlfaClient;
    const conv = new ConversationState(client, 's');
    const first = conv.load();
    const second = conv.load();
    expect(answers).toHaveLength(1);
    // Zdarzenie w trakcie: przy dwóch wczytaniach trafiało do drzewa zastępowanego drugą migawką.
    conv.apply({ type: 'Stop', session_id: 's', turn_id: streaming.id, reason: 'end' });
    answers[0]?.({ turns: [streaming], annotations: {} });
    await Promise.all([first, second]);
    expect(conv.turn(streaming.id)?.status).toBe('complete');
    // Po zakończeniu kolejne wczytanie znów pyta rdzeń.
    const again = conv.load();
    expect(answers).toHaveLength(2);
    answers[1]?.({ turns: [streaming], annotations: {} });
    await again;
  });

  it('usunięcie aktywnej sesji otwiera następną tylko raz, choć zdarzenie o usunięciu już czeka', async () => {
    const { app } = setupApp();
    await app.start();
    const active = app.activeId ?? '';
    const open = vi.spyOn(app, 'openSession');
    const removing = app.sessionRemoved(active);
    // To samo usunięcie wraca zdarzeniem, zanim pierwsze sprzątanie się skończy.
    const again = app.sessionRemoved(active);
    await Promise.all([removing, again]);
    expect(open).toHaveBeenCalledTimes(1);
    expect(app.activeId).not.toBe(active);
  });

  it('blokada ponownego uruchomienia jest odpytywana z rdzenia w obie strony', async () => {
    const view = (blocked: string | null) =>
      ({
        phase: 'ready',
        restart_blocked: blocked ? { pl: blocked, en: blocked } : null,
      }) as unknown as Awaited<ReturnType<UpdatesApi['status']>>;
    const updates = new UpdatesState();
    let blocked: string | null = 'trwa rozmowa głosowa';
    const api = { status: () => Promise.resolve(view(blocked)) } as unknown as UpdatesApi;
    // Pierwsze „ready” przychodzi zdarzeniem bez powodu, a agentka jest zajęta.
    updates.applyEvent(view(null));
    expect(updates.view?.restart_blocked).toBeNull();
    await updates.refreshRestartBlock(api);
    expect(updates.view?.restart_blocked).not.toBeNull();
    // Rozmowa się skończyła — żadne zdarzenie o tym nie mówi.
    blocked = null;
    await updates.refreshRestartBlock(api);
    expect(updates.view?.restart_blocked).toBeNull();
  });

  it('prawy panel przeciągnięty dokładnie na szerokość domyślną nie przeskakuje na szerokim oknie', () => {
    const layout = new LayoutState();
    const fallback = layout.rightWidth;
    layout.setRightWidth(fallback);
    expect(layout.rightWidth).not.toBe(fallback);
    expect(Math.abs(layout.rightWidth - fallback)).toBe(1);
  });
});
