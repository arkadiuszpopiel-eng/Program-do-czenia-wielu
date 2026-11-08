// Dziennik rozruchu: po starcie UI werdykt z `boot_report` trafia do stanu i do toastu (OK znika,
// problem zostaje z „Pokaż dziennik” → Zdrowie systemu). Prośba właściciela 2026-10-22.
import { describe, expect, it } from 'vitest';
import { BOOT_PAGE, boot, bootSeconds, watchBoot } from '../boot.svelte';
import { setupApp } from './helpers';

const noWait = () => Promise.resolve();

describe('watchBoot', () => {
  it('rozruch OK: krótki toast sukcesu z czasem', async () => {
    const { app } = setupApp({ scenario: 'boot-ok' });
    await watchBoot(app, noWait);
    expect(boot.report?.verdict).toBe('ok');
    const toast = app.toasts.items.at(-1);
    expect(toast?.kind).toBe('success');
    expect(toast?.message).toContain('Rozruch OK');
    expect(toast?.message).toContain('2,8 s');
  });

  it('z uwagami: toast informacyjny z liczbą ostrzeżeń i przejściem do dziennika', async () => {
    const { app } = setupApp({ scenario: 'boot-warnings' });
    await watchBoot(app, noWait);
    const toast = app.toasts.items.at(-1);
    expect(toast?.kind).toBe('info');
    expect(toast?.message).toContain('ostrzeżenia: 2');
    expect(toast?.actionLabel).toBe('Pokaż dziennik');
  });

  it('z problemem: toast błędu zostaje i otwiera Zdrowie systemu', async () => {
    const { app } = setupApp({ scenario: 'boot-errors' });
    await watchBoot(app, noWait);
    const toast = app.toasts.items.at(-1);
    expect(toast?.kind).toBe('error');
    expect(toast?.timeoutMs).toBe(0);
    expect(toast?.message).toContain('błędy: 1');
    toast?.onAction?.();
    expect(app.settingsPage).toBe(BOOT_PAGE);
  });

  it('czeka, aż okno rozruchu się zamknie', async () => {
    const { app } = setupApp();
    let calls = 0;
    const real = app.client.system.bootReport.bind(app.client.system);
    app.client.system.bootReport = async () => {
      calls++;
      const r = await real();
      return { ...r, finished: calls >= 3 };
    };
    await watchBoot(app, noWait);
    expect(calls).toBe(3);
    expect(app.toasts.items.length).toBe(1);
  });

  it('okno rozruchu niezamknięte (domyślna atrapa): bez toastu', async () => {
    const { app } = setupApp();
    await watchBoot(app, noWait);
    expect(boot.report?.finished).toBe(false);
    expect(app.toasts.items.length).toBe(0);
  });

  it('sekundy po polsku i angielsku', () => {
    expect(bootSeconds('pl', 2840)).toBe('2,8');
    expect(bootSeconds('en', 2840)).toBe('2.8');
  });
});
