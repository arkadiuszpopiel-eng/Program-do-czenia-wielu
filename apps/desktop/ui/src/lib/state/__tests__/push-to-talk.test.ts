// Spacja „przytrzymaj, aby mówić” (`voice.ptt`): wciśnięcie i puszczenie Spacji w trybie PTT idą do
// rdzenia (`voice_ptt`), autopowtórzenie nic nie wysyła, pole tekstowe i przyciski zachowują Spację,
// tryb „Przełącznik” jej nie przejmuje, utrata fokusu okna kończy mówienie.
import { describe, expect, it, vi } from 'vitest';
import { PushToTalk } from '../push-to-talk';
import { setupApp } from './helpers';

/** Element-atrapa (testy bez DOM): `closest` dopasowuje sam znacznik. */
const element = (tag: string | null) => ({
  closest: (selector: string) =>
    tag !== null && selector.split(',').some((s) => s.trim() === tag) ? {} : null,
});

function space(type: 'down' | 'up', extra: Partial<KeyboardEvent> = {}) {
  let prevented = false;
  return {
    type,
    code: 'Space',
    key: ' ',
    repeat: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    isComposing: false,
    target: element(null),
    ...extra,
    get defaultPrevented() {
      return prevented;
    },
    preventDefault() {
      prevented = true;
    },
  } as unknown as KeyboardEvent;
}

async function started(mode: 'ptt' | 'toggle') {
  const { app, client } = setupApp();
  await app.start();
  app.view = 'chat';
  app.settings['voice.mode'] = mode;
  const calls: boolean[] = [];
  vi.spyOn(client.voice, 'ptt').mockImplementation((pressed: boolean) => {
    calls.push(pressed);
    return Promise.resolve();
  });
  return { app, ptt: new PushToTalk(app), calls };
}

describe('voice.ptt — Spacja „przytrzymaj, aby mówić”', () => {
  it('wciśnięcie włącza, autopowtórzenie nic nie wysyła, puszczenie wyłącza', async () => {
    const { ptt, calls } = await started('ptt');
    const down = space('down');
    ptt.down(down);
    expect(down.defaultPrevented).toBe(true);
    ptt.down(space('down', { repeat: true }));
    ptt.up(space('up'));
    expect(calls).toEqual([true, false]);
    expect(ptt.held).toBe(false);
  });

  it('w polu tekstowym i na przycisku Spacja zostaje zwykłym klawiszem', async () => {
    const { ptt, calls } = await started('ptt');
    for (const tag of ['textarea', 'input', 'button']) {
      const down = space('down', { target: element(tag) as unknown as EventTarget });
      ptt.down(down);
      ptt.up(space('up', { target: element(tag) as unknown as EventTarget }));
      expect(down.defaultPrevented, tag).toBe(false);
    }
    expect(calls).toEqual([]);
  });

  it('tryb „Przełącznik”, modyfikator albo inny widok — bez mówienia', async () => {
    const { app, ptt, calls } = await started('toggle');
    ptt.down(space('down'));
    app.settings['voice.mode'] = 'ptt';
    ptt.down(space('down', { ctrlKey: true }));
    app.view = 'settings';
    ptt.down(space('down'));
    expect(calls).toEqual([]);
  });

  it('utrata fokusu okna z wciśniętą Spacją kończy mówienie', async () => {
    const { ptt, calls } = await started('ptt');
    ptt.down(space('down'));
    ptt.release();
    ptt.release();
    expect(calls).toEqual([true, false]);
  });

  it('błąd rdzenia (brak modeli mowy) — komunikat zamiast ciszy', async () => {
    const { app, ptt } = await started('ptt');
    vi.mocked(app.client.voice.ptt).mockRejectedValueOnce(
      new Error('Brak modelu rozpoznawania mowy'),
    );
    ptt.down(space('down'));
    await vi.waitFor(() => expect(app.toasts.items.length).toBeGreaterThan(0));
    expect(app.toasts.items.at(-1)?.message).toContain('Brak modelu');
  });
});
