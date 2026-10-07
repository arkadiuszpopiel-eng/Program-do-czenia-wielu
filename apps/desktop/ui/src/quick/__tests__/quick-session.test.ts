// Szybkie pytanie: zdarzenia odpowiedzi mogą wyprzedzić wynik `quick_ask` (paczka zdarzeń rdzenia
// i odpowiedź IPC to dwa niezależne kanały) — pierwsza odpowiedź nie może zginąć (uwaga Q-2).
import { describe, expect, it, vi } from 'vitest';
import { FakeAlfaClient, VirtualScheduler } from '../../lib/api/fake/fake-client';
import { QuickSession } from '../quick-session.svelte';

const flush = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

/** Atrapa z opóźnioną odpowiedzią komend: zdarzenia docierają, zanim wróci wynik `quick_ask`. */
function setup() {
  const scheduler = new VirtualScheduler();
  const client = new FakeAlfaClient({ scheduler, latencyMs: 50 });
  const quick = new QuickSession(client);
  client.subscribe((batch) => quick.apply(batch));
  /** Pytanie → zdarzenia (przed wynikiem) → wynik → strumień do końca. */
  const askAndStream = async (text: string): Promise<void> => {
    const asked = quick.ask(text);
    await flush();
    scheduler.advance(50);
    await asked;
    scheduler.advance(30_000);
    await flush();
  };
  return { scheduler, client, quick, askAndStream };
}

describe('QuickSession', () => {
  it('pierwsza odpowiedź: zdarzenia sprzed wyniku quick_ask trafiają do odpowiedzi', async () => {
    const { quick, askAndStream } = setup();
    await askAndStream('Ile to 2+2?');
    expect(quick.sessionId).toBe('s-quick');
    expect(quick.queued).toBe(false);
    expect(quick.answer?.author).not.toBe('user');
    expect(quick.answer?.status).toBe('complete');
    expect(quick.answer?.blocks.length).toBeGreaterThan(0);
  });

  it('kolejne pytanie w tej samej sesji: nowa odpowiedź, tekst bez zdublowania', async () => {
    const { client, quick, askAndStream } = setup();
    await askAndStream('Ile to 2+2?');
    const first = quick.answer?.id;
    await askAndStream('A 3+3?');
    expect(quick.answer?.id).not.toBe(first);
    const stored = client.core.findTurn(quick.answer?.id ?? '');
    expect(quick.answer?.status).toBe('complete');
    expect(quick.answer?.text).toBe(stored?.text);
  });

  it('zdarzenia innych sesji w trakcie żądania nie trafiają do odpowiedzi', async () => {
    const { scheduler, client, quick } = setup();
    const asked = quick.ask('Ile to 2+2?');
    void client.turns.send('s-q3', {
      parent_id: null,
      text: 'Inna',
      addressed_to: null,
      profile: null,
    });
    await flush();
    scheduler.advance(50);
    await asked;
    scheduler.advance(30_000);
    await flush();
    expect(quick.answer?.session_id).toBe('s-quick');
  });

  it('odrzucone pytanie: błąd dla okna, bufor zwolniony, kolejne pytanie działa', async () => {
    const { quick, client, askAndStream } = setup();
    vi.spyOn(client.quick, 'ask').mockRejectedValueOnce(new Error('Brak połączenia z rdzeniem'));
    await expect(quick.ask('Ile to 2+2?')).rejects.toThrow('Brak połączenia z rdzeniem');
    expect(quick.answer).toBeNull();
    await askAndStream('Ile to 2+2?');
    expect(quick.answer?.status).toBe('complete');
  });
});
