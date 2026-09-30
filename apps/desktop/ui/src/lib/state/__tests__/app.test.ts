import { beforeAll, describe, expect, it } from 'vitest';
import { handleKeydown, runCommand } from '../commands';
import { keyEvent, setupApp } from './helpers';

beforeAll(() => {
  globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) =>
    setTimeout(() => cb(0), 0) as unknown as number) as typeof requestAnimationFrame;
});

describe('AppState', () => {
  it('start: sesje, aktywna rozmowa, stan systemu, koszty', async () => {
    const { app } = setupApp();
    await app.start();
    expect(app.view).toBe('chat');
    expect(app.activeId).toBe('s-q3');
    expect(app.conversation?.path.length).toBe(6);
    expect(app.system?.online).toBe(true);
    expect(app.costs?.session.minor).toBe(108);
    expect(app.sessions.groups.map((g) => g.kind)).toEqual(['pinned', 'project', 'loose']);
  });

  it('pierwsze uruchomienie → onboarding', async () => {
    const { app } = setupApp({ scenario: 'first-run' });
    await app.start();
    expect(app.view).toBe('onboarding');
    expect(app.activeId).toBeNull();
  });

  it('strumień trafia do stanu paczkami — raz na klatkę', async () => {
    const { app, frames, scheduler } = setupApp();
    await app.start();
    await app.conversation?.send('Zaplanuj tydzień', null, null);
    scheduler.advance(300);
    await new Promise((r) => setTimeout(r, 0));
    expect(frames.scheduled).toBe(1);
    const before = app.conversation?.path.length;
    frames.tick();
    expect(app.conversation?.path.length).toBe((before ?? 0) + 2);
    expect(frames.scheduled).toBe(0);
  });

  it('usunięcie sesji → toast „Cofnij" przywraca sesję', async () => {
    const { app, advance } = setupApp();
    await app.start();
    await app.deleteSession('s-trip');
    expect(app.sessions.list.some((s) => s.id === 's-trip')).toBe(false);
    const toast = app.toasts.items.at(-1);
    expect(toast?.actionLabel).toBe('Cofnij');
    if (toast) app.toasts.act(toast.id);
    await advance(0);
    expect(app.sessions.list.some((s) => s.id === 's-trip')).toBe(true);
  });

  it('przełączenie sesji 1000 wiadomości', async () => {
    const { app } = setupApp();
    await app.start();
    const t0 = performance.now();
    await app.openSession('s-long');
    const elapsed = performance.now() - t0;
    expect(app.conversation?.path).toHaveLength(1000);
    expect(elapsed).toBeLessThan(150);
  });

  it('język: przełączenie na EN ładuje słownik leniwie', async () => {
    const { app } = setupApp();
    await app.start();
    await app.setSetting('ui.locale', 'en');
    expect(app.i18n.t('sessions.title')).toBe('Sessions');
    expect(app.i18n.t('files.versions', { n: 2 })).toBe('2 versions');
    await app.setSetting('ui.locale', 'pl');
    expect(app.i18n.t('files.versions', { n: 2 })).toBe('2 wersje');
  });

  it('skróty: Ctrl+B, Ctrl+K, F5 zablokowane, Alt+2 → Oś czasu, Ctrl+, → ustawienia', async () => {
    const { app } = setupApp();
    await app.start();
    app.layout.width = 1600;
    const leftBefore = app.layout.current(app.activeId).left_open;
    handleKeydown(app, keyEvent('b', 'KeyB', { ctrlKey: true }));
    expect(app.layout.current(app.activeId).left_open).toBe(!leftBefore);
    handleKeydown(app, keyEvent('k', 'KeyK', { ctrlKey: true }));
    expect(app.palette.open).toBe(true);
    const f5 = keyEvent('F5', 'F5');
    handleKeydown(app, f5);
    expect(f5.defaultPrevented).toBe(true);
    handleKeydown(app, keyEvent('2', 'Digit2', { altKey: true }));
    expect(app.layout.current(app.activeId).right_tab).toBe('timeline');
    handleKeydown(app, keyEvent(',', 'Comma', { ctrlKey: true }));
    expect(app.view).toBe('settings');
    handleKeydown(app, keyEvent('Escape', 'Escape'));
    expect(app.palette.open).toBe(true);
    app.palette.open = false;
    handleKeydown(app, keyEvent('Escape', 'Escape'));
    expect(app.view).toBe('chat');
  });

  it('Esc zatrzymuje generowanie, gdy nic innego nie jest otwarte', async () => {
    const { app, advance } = setupApp();
    await app.start();
    app.layout.width = 1600;
    await app.conversation?.send('Napisz szczegółowy esej', null, null);
    await advance(100);
    expect(app.conversation?.streaming).toBeDefined();
    const esc = keyEvent('Escape', 'Escape');
    handleKeydown(app, esc);
    expect(esc.defaultPrevented).toBe(true);
    await advance(100);
    expect(app.conversation?.streaming).toBeUndefined();
  });

  it('zoom i nadpisanie skrótu z wykrywaniem', async () => {
    const { app } = setupApp();
    await app.start();
    runCommand(app, 'view.zoomIn');
    runCommand(app, 'view.zoomIn');
    expect(app.num('ui.zoom', 100)).toBe(120);
    runCommand(app, 'view.zoomReset');
    expect(app.num('ui.zoom', 100)).toBe(100);
    await app.setShortcut('panel.sessions', 'Ctrl+J');
    expect(app.keymap.get('Ctrl+J')?.id).toBe('panel.sessions');
    expect(app.keymap.get('Ctrl+B')).toBeUndefined();
  });

  it('offline: wiadomość w kolejce, baner; po powrocie wysłana', async () => {
    const { app, client, advance } = setupApp({ scenario: 'offline' });
    await app.start();
    await app.conversation?.send('test', null, null);
    await advance(0);
    expect(app.conversation?.path.at(-1)?.status).toBe('queued');
    expect(app.system?.queued_messages).toBe(1);
    client.setOnline(true);
    await advance(5000);
    expect(app.system?.queued_messages).toBe(0);
    expect(app.conversation?.path.at(-2)?.status).toBe('complete');
  });
});
