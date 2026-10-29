import { describe, expect, it } from 'vitest';
import { applyJuniorVoice, JUNIOR_SPEED, offerJunior } from '../junior';
import { ToastState } from '../toasts.svelte';
import type { AppState } from '../app.svelte';

/** Minimalny stan okna: ustawienia, toasty i zapis. */
function fakeApp(settings: Record<string, number | boolean>) {
  const toasts = new ToastState({ setTimeout: () => 0, clearTimeout: () => undefined });
  const saved: [string, unknown][] = [];
  const app = {
    toasts,
    i18n: { t: (key: string, vars?: Record<string, unknown>) => `${key}:${vars?.['speed'] ?? ''}` },
    num: (k: string, d: number) => (typeof settings[k] === 'number' ? (settings[k] as number) : d),
    bool: (k: string, d: boolean) =>
      typeof settings[k] === 'boolean' ? (settings[k] as boolean) : d,
    setSetting: (k: string, v: number | boolean) => {
      saved.push([k, v]);
      settings[k] = v;
      return Promise.resolve(true);
    },
  };
  return { app: app as unknown as AppState, toasts, saved };
}

describe('Junior: propozycja wolniejszej mowy i dźwięków', () => {
  it('proponuje, a „Włącz” ustawia tempo 85 % i dźwięki', async () => {
    const { app, toasts, saved } = fakeApp({ 'voice.tts_speed': 100, 'notify.earcons': false });
    offerJunior(app);
    expect(toasts.items.map((t) => t.message)).toEqual([`junior.offer:${JUNIOR_SPEED}`]);
    await applyJuniorVoice(app);
    expect(saved).toEqual([
      ['voice.tts_speed', 85],
      ['notify.earcons', true],
    ]);
    // Ten sam klucz: potwierdzenie zastępuje propozycję.
    expect(toasts.items.map((t) => t.message)).toEqual([`junior.applied:${JUNIOR_SPEED}`]);
  });

  it('nic nie proponuje, gdy głos już jest wolniejszy, a dźwięki włączone', () => {
    const { app, toasts } = fakeApp({ 'voice.tts_speed': 80, 'notify.earcons': true });
    offerJunior(app);
    expect(toasts.items).toEqual([]);
  });

  it('nie przyspiesza mowy ustawionej wolniej niż 85 %', async () => {
    const { app, saved } = fakeApp({ 'voice.tts_speed': 70, 'notify.earcons': false });
    await applyJuniorVoice(app);
    expect(saved).toEqual([['notify.earcons', true]]);
  });
});
