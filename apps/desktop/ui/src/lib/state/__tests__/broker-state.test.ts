import { describe, expect, it } from 'vitest';
import type { BrokerStatusView } from '../../api/types-broker';
import { BrokerState } from '../broker.svelte';

const view = (state: BrokerStatusView['state']): BrokerStatusView => ({
  mode: 'service',
  state,
  approval_window: state === 'connected',
  watchdog: true,
  isolated: state === 'connected',
  detail: null,
});

describe('BrokerState (audyt A-12)', () => {
  it('migawka z odczytu nie nadpisuje nowszego zdarzenia', async () => {
    const broker = new BrokerState();
    let resolve: (v: BrokerStatusView) => void = () => {};
    const load = broker.load({ status: () => new Promise((r) => (resolve = r)) });
    broker.apply(view('lost'));
    resolve(view('connected'));
    await load;
    expect(broker.view?.state).toBe('lost');
    expect(broker.banner).toBe('lost');
  });

  it('nieudany odczyt bez widoku — baner „stan nieznany”; zdarzenie go zastępuje', async () => {
    const broker = new BrokerState();
    await broker.load({ status: () => Promise.reject(new Error('brak komendy')) });
    expect(broker.banner).toBe('unknown');
    broker.apply(view('connected'));
    expect(broker.banner).toBeNull();
    // Kolejny nieudany odczyt nie kasuje widoku ze zdarzenia.
    await broker.load({ status: () => Promise.reject(new Error('chwilowy błąd')) });
    expect(broker.view?.state).toBe('connected');
    expect(broker.banner).toBeNull();
  });
});
