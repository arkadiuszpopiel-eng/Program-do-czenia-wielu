import { beforeAll, describe, expect, it } from 'vitest';
import { setupApp } from './helpers';

beforeAll(() => {
  globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) =>
    setTimeout(() => cb(0), 0) as unknown as number) as typeof requestAnimationFrame;
});

describe('Zdarzenia rdzenia → stan UI', () => {
  it('OpenSession przełącza widok na wskazaną sesję (zasobnik, alfa://, Szybkie pytanie)', async () => {
    const { app, client, advance } = setupApp();
    await app.start();
    app.openSettings('general');
    client.core.emit([{ type: 'OpenSession', session_id: 's-trip' }]);
    await advance(0);
    await advance(0);
    expect(app.activeId).toBe('s-trip');
    expect(app.view).toBe('chat');
  });

  it('LocalModelProgress: postęp pobierania modelu lokalnego aż do „gotowy"', async () => {
    const { app, client, advance } = setupApp({ scenario: 'no-keys' });
    await app.start();
    const [model] = await client.models.localList();
    expect(model?.installed).toBe(false);
    await client.models.localDownload(null);
    await advance(0);
    expect(app.localDownload?.state).toBe('downloading');
    await advance(600);
    expect(app.localDownload?.bytes).toBeGreaterThan(0);
    await advance(2_000);
    expect(app.localDownload?.state).toBe('done');
    expect((await client.models.localList())[0]?.installed).toBe(true);
  });

  it('poziom autonomii: obniżenie od razu (applied), podniesienie — okno Brokera', async () => {
    const { client } = setupApp();
    const lowered = await client.permissions.requestLevel('L1', null);
    expect(lowered.status).toBe('applied');
    expect((await client.permissions.get(null)).global).toBe('L1');
    const raised = await client.permissions.requestLevel('L4', null);
    expect(raised.status).toBe('opened_broker');
    expect((await client.permissions.get(null)).global).toBe('L1');
  });

  it('sekrety nigdy w paczce: brak eksportu sekretów (CX-a)', () => {
    const { client } = setupApp();
    expect('exportSecrets' in client.transfer).toBe(false);
  });
});
