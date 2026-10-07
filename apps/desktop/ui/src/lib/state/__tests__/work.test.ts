import { beforeAll, describe, expect, it } from 'vitest';
import { setupApp } from './helpers';

beforeAll(() => {
  globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) =>
    setTimeout(() => cb(0), 0) as unknown as number) as typeof requestAnimationFrame;
});

describe('Stan F8: Ekran, terminal, zdrowie, podprzebiegi', () => {
  it('start pobiera stan sterowania; GuiActivity aktualizuje wskaźnik', async () => {
    const { app, client, advance } = setupApp();
    await app.start();
    await advance(0);
    expect(app.work.controlling).toBe(true);
    await client.gui.stop();
    await advance(0);
    expect(app.work.gui?.taken_over).toBe(true);
    expect(app.work.controlling).toBe(false);
    await client.gui.release();
    await advance(0);
    expect(app.work.gui?.taken_over).toBe(false);
  });

  it('HealthChanged → skrót zdrowia; prośba o terminal z gestu', async () => {
    const { app, client, advance } = setupApp();
    await app.start();
    await client.health.approve(1);
    await advance(0);
    expect(app.work.health).toEqual({ overall: 'degraded', pending: 1 });
    app.work.openTerminal('claude_login');
    const first = app.work.terminal;
    app.work.openTerminal('claude_login');
    expect(app.work.terminal?.seq).toBe((first?.seq ?? 0) + 1);
    app.work.closeTerminal();
    expect(app.work.terminal).toBeNull();
  });

  it('podprzebieg (Krytyczka) nie zastępuje przebiegu głównego sesji', async () => {
    const { app, client } = setupApp();
    const [main, child] = await client.agents.runs('s-q3');
    expect(child?.run.parent_id).toBe(main?.run.id);
    expect(child?.run.label).toBe('Krytyczka');
    if (!main || !child) throw new Error('brak przebiegów atrapy');
    app.runs.update(main.run);
    app.runs.update(child.run);
    expect(app.runs.bySession['s-q3']?.id).toBe(main.run.id);
  });
});
