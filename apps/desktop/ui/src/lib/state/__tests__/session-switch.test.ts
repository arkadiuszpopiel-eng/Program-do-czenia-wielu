// Szybkie przełączanie sesji A → B: spóźnione odpowiedzi dla A nie nadpisują kosztów B ani
// aktywnej sesji w rdzeniu (uwaga przeglądu PR #1: Q-10).
import { beforeAll, describe, expect, it, vi } from 'vitest';
import type { CostSummary } from '../../api/types';
import { flush, setupApp } from './helpers';

beforeAll(() => {
  globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) =>
    setTimeout(() => cb(0), 0) as unknown as number) as typeof requestAnimationFrame;
});

/** Odpowiedź `costs_summary` dla `slow`, wstrzymana do wywołania `release()`. */
function holdCosts(client: ReturnType<typeof setupApp>['client'], slow: string) {
  const original = client.costs.summary.bind(client.costs);
  let release = (): void => undefined;
  const spy = vi.spyOn(client.costs, 'summary').mockImplementation(async (id) => {
    const summary = await original(id);
    if (id !== slow) return summary;
    await new Promise<void>((resolve) => (release = resolve));
    return { ...summary, session: { minor: 99_999, currency: 'PLN' } } satisfies CostSummary;
  });
  return { spy, release: () => release() };
}

describe('przełączanie sesji — aktualność odpowiedzi', () => {
  it('openSession: spóźnione koszty A nie trafiają do B, rdzeń kończy z aktywną B', async () => {
    const { app, client } = setupApp();
    await app.start();
    const active = vi.spyOn(client.app, 'setActiveSession');
    const held = holdCosts(client, 's-api');
    const openA = app.openSession('s-api');
    await app.openSession('s-trip');
    expect(app.activeId).toBe('s-trip');
    held.release();
    await openA;
    await flush();
    expect(app.activeId).toBe('s-trip');
    expect(app.costs?.session.minor).not.toBe(99_999);
    expect(active.mock.calls.at(-1)).toEqual(['s-trip']);
    expect(client.core.activeSession).toBe('s-trip');
  });

  it('refreshCosts: wynik dla poprzednio aktywnej sesji jest pomijany', async () => {
    const { app, client } = setupApp();
    await app.start();
    await app.openSession('s-api');
    const held = holdCosts(client, 's-api');
    const stale = app.refreshCosts();
    await app.openSession('s-trip');
    const fresh = app.costs;
    held.release();
    await stale;
    expect(app.costs).toBe(fresh);
    expect(app.costs?.session.minor).not.toBe(99_999);
  });
});
