// Zdarzenia rdzenia, które tylko podmieniają stan okna (obsada, koszty, stan systemu, Broker,
// raport Marszałka): dług z założenia bramki katalogu funkcji 31.10 (docs/plan/45).
import { beforeAll, describe, expect, it } from 'vitest';
import type { AlfaEvent } from '../../api/types-system';
import { setupApp } from './helpers';

beforeAll(() => {
  globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) =>
    setTimeout(() => cb(0), 0) as unknown as number) as typeof requestAnimationFrame;
});

async function started() {
  const ctx = setupApp();
  await ctx.app.start();
  await ctx.advance(0);
  const emit = async (event: AlfaEvent) => {
    ctx.client.core.emit([event]);
    await ctx.advance(0);
    await ctx.advance(0);
  };
  return { ...ctx, emit };
}

describe('Zdarzenia stanu → okno główne', () => {
  it('AgentsChanged podmienia obsadę wskazanej sesji', async () => {
    const { app, emit } = await started();
    const sid = app.activeId ?? '';
    await emit({ type: 'AgentsChanged', session_id: sid, agents: [] });
    expect(app.agents[sid]).toEqual([]);
    await emit({ type: 'AgentsChanged', session_id: 's-inna', agents: [] });
    expect(app.agents['s-inna']).toEqual([]);
  });

  it('CostsChanged: koszty otwartej sesji i ogólne trafiają do paska, cudze nie', async () => {
    const { app, emit } = await started();
    const base = app.costs;
    expect(base).not.toBeNull();
    if (!base) return;
    const other = { ...base, session: { ...base.session, minor: 12345 } };
    await emit({ type: 'CostsChanged', session_id: 's-cudza', costs: other });
    expect(app.costs).toBe(base);
    await emit({ type: 'CostsChanged', session_id: app.activeId ?? '', costs: other });
    expect(app.costs).toEqual(other);
    const general = { ...base, day: { ...base.day, minor: 700 } };
    await emit({ type: 'CostsChanged', session_id: '', costs: general });
    expect(app.costs).toEqual(general);
  });

  it('SystemStatusChanged podmienia stan systemu (połączenie, kolejka)', async () => {
    const { app, emit } = await started();
    const status = app.system;
    expect(status).not.toBeNull();
    if (!status) return;
    await emit({
      type: 'SystemStatusChanged',
      status: { ...status, online: false, queued_messages: 3 },
    });
    expect(app.system?.online).toBe(false);
    expect(app.system?.queued_messages).toBe(3);
  });

  it('BrokerStatus: nowy widok Sędziego Cieni zastępuje migawkę i zdejmuje błąd odczytu', async () => {
    const { app, emit } = await started();
    const view = app.broker.view;
    expect(view).not.toBeNull();
    if (!view) return;
    app.broker.failed = true;
    await emit({ type: 'BrokerStatus', status: { ...view } });
    expect(app.broker.failed).toBe(false);
    expect(app.broker.view).toEqual(view);
  });

  it('MarshalReportReady: raport dnia jako powiadomienie i dla słuchaczy', async () => {
    const { app, emit } = await started();
    const seen: string[] = [];
    app.on((e) => {
      if (e.type === 'MarshalReportReady') seen.push(e.report.day);
    });
    const report = {
      day: '2026-10-31',
      escalations: 0,
      failed: 1,
      submitted: 4,
      succeeded: 3,
      text: '3 z 4 zadań gotowe',
    };
    await emit({ type: 'MarshalReportReady', report });
    expect(app.toasts.items.some((t) => t.message.includes('3 z 4 zadań gotowe'))).toBe(true);
    expect(seen).toEqual(['2026-10-31']);
  });
});
