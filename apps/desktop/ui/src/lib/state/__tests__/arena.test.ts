// Arena na atrapie: dwie odpowiedzi jako nowe gałęzie pod tą samą wiadomością (po kolei — jedna
// generacja na sesję), model w `Usage`, wybór gałęzi i błąd tury (limit kosztów).
import { describe, expect, it } from 'vitest';
import { FakeAlfaClient, VirtualScheduler, type FakeScenario } from '../../api/fake/fake-client';
import { isChatEvent } from '../../logic/apply-event';
import { arenaStep, type ArenaSlot } from '../../logic/arena';
import { ConversationState } from '../conversation.svelte';
import { flush } from './helpers';

function setup(scenario: FakeScenario = 'default') {
  const scheduler = new VirtualScheduler();
  const client = new FakeAlfaClient({ scheduler, scenario });
  const conv = new ConversationState(client, 's-q3');
  client.subscribe((batch) => {
    for (const e of batch) if (isChatEvent(e) && e.session_id === 's-q3') conv.apply(e);
  });
  const run = async (ms: number) => {
    scheduler.advance(ms);
    await flush();
  };
  /** Prowadzi arenę jak okno: kolejne stanowisko po zakończeniu poprzedniego. */
  const arena = async (turnId: string, models: string[]): Promise<ArenaSlot[]> => {
    const slots: ArenaSlot[] = models.map((model) => ({ model, turnId: null, error: null }));
    for (let i = 0; i < 100; i++) {
      const step = arenaStep(slots, (id) => conv.turn(id)?.status, false);
      if (step.kind === 'done') break;
      const slot = step.kind === 'start' ? slots[step.index] : undefined;
      if (slot) {
        await conv.arenaAnswer(turnId, slot.model).then(
          (id) => {
            slot.turnId = id;
          },
          (e: unknown) => {
            slot.error = String(e);
          },
        );
      }
      await run(500);
    }
    return slots;
  };
  return { client, conv, arena };
}

describe('Arena (atrapa)', () => {
  it('dwie odpowiedzi → dwie nowe gałęzie obok poprzedniej, stara nietknięta', async () => {
    const { conv, arena } = setup();
    await conv.load();
    const before = conv.turn('q6')?.text;
    const slots = await arena('q5', ['anthropic:claude-opus-5-5', 'openai:gpt-6-sol']);
    const [a, b] = slots.map((s) => (s.turnId ? conv.turn(s.turnId) : undefined));
    expect(a?.status).toBe('complete');
    expect(b?.status).toBe('complete');
    expect(a?.parent_id).toBe('q5');
    expect(b?.parent_id).toBe('q5');
    expect(a?.usage?.model).toBe('claude-opus-5-5');
    expect(b?.usage).toMatchObject({ model: 'gpt-6-sol', provider: 'OpenAI' });
    expect(a?.text).not.toBe(b?.text);
    expect(b && conv.siblings(b)).toEqual({ index: 3, total: 3 });
    expect(conv.turn('q6')?.text).toBe(before);
  });

  it('„Wybierz tę odpowiedź” → gałąź pierwszej staje się bieżącą ścieżką', async () => {
    const { client, conv, arena } = setup();
    await conv.load();
    const slots = await arena('q6', ['anthropic:claude-opus-5-5', 'openai:gpt-6-sol']);
    const first = slots[0]?.turnId ?? '';
    expect(conv.path.at(-1)?.id).toBe(slots[1]?.turnId);
    expect(conv.selectTurnById(first)).toBe(true);
    expect(conv.path.at(-1)?.id).toBe(first);
    await expect(client.turns.selectBranch('s-q3', first)).resolves.toBeUndefined();
    expect(conv.selectTurnById('brak')).toBe(false);
  });

  it('limit kosztów: tura z błędem, druga odpowiedź i tak powstaje', async () => {
    const { conv, arena } = setup('arena-budget');
    await conv.load();
    const slots = await arena('q5', ['openai:gpt-6-sol', 'anthropic:claude-opus-5-5']);
    const [a, b] = slots.map((s) => (s.turnId ? conv.turn(s.turnId) : undefined));
    expect(a?.status).toBe('error');
    expect(a?.error?.code).toBe('budget_blocked');
    expect(b?.status).toBe('complete');
  });

  it('model spoza zapisu dostawca:model jest odrzucany', async () => {
    const { client } = setup();
    await expect(client.turns.arenaAnswer('s-q3', 'q5', 'auto')).rejects.toThrow('dostawca:model');
  });
});
