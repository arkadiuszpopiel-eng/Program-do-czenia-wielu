import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AutonomyLevel, Turn } from '../../api/types';
import { acceptPlan, showsPlanActions, stopPlanWatch } from '../plan-mode';

const turn = (over: Partial<Turn>): Turn =>
  ({
    id: 't2',
    author: 'delta',
    status: 'complete',
    approval: null,
    ...over,
  }) as Turn;

describe('tryb „Plan” (Z.5)', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    stopPlanWatch();
    vi.useRealTimers();
  });

  it('trzy odpowiedzi tylko pod ostatnią zakończoną odpowiedzią agentki w rozmowie na L0', () => {
    const last = turn({});
    expect(showsPlanActions(last, last, 'L0')).toBe(true);
    expect(showsPlanActions(last, last, 'L1')).toBe(false);
    expect(showsPlanActions(last, last, undefined)).toBe(false);
    expect(showsPlanActions(turn({ id: 't1' }), last, 'L0')).toBe(false);
    const user = turn({ author: 'user' });
    expect(showsPlanActions(user, user, 'L0')).toBe(false);
    const streaming = turn({ status: 'streaming' });
    expect(showsPlanActions(streaming, streaming, 'L0')).toBe(false);
    const waiting = turn({ approval: {} as Turn['approval'] });
    expect(showsPlanActions(waiting, waiting, 'L0')).toBe(false);
  });

  it('akceptacja czeka na zgodę Sędziego, a wykonanie wysyła raz — po przyjęciu poziomu', async () => {
    let level: AutonomyLevel = 'L0';
    const execute = vi.fn(() => Promise.resolve());
    const notify = vi.fn();
    const request = vi.fn(() =>
      Promise.resolve({ status: 'opened_broker' as const, request_id: '9' }),
    );
    await acceptPlan('auto', {
      request,
      readLevel: () => Promise.resolve(level),
      execute,
      notify,
    });
    expect(request).toHaveBeenCalledWith('L3');
    expect(notify).toHaveBeenCalledWith('requested', 'L3');
    await vi.advanceTimersByTimeAsync(6_000);
    expect(execute).not.toHaveBeenCalled();
    level = 'L3';
    await vi.advanceTimersByTimeAsync(2_000);
    expect(execute).toHaveBeenCalledTimes(1);
    expect(notify).toHaveBeenCalledWith('started', 'L3');
    await vi.advanceTimersByTimeAsync(10_000);
    expect(execute).toHaveBeenCalledTimes(1);
  });

  it('„zatwierdzam ręcznie” prosi o L1; poziom przyjęty od razu — wykonanie bez czekania', async () => {
    const execute = vi.fn(() => Promise.resolve());
    const request = vi.fn(() => Promise.resolve({ status: 'applied' as const, request_id: '' }));
    await acceptPlan('manual', {
      request,
      readLevel: () => Promise.resolve('L1'),
      execute,
      notify: vi.fn(),
    });
    expect(request).toHaveBeenCalledWith('L1');
    expect(execute).toHaveBeenCalledTimes(1);
  });

  it('nowa akceptacja przerywa czekanie na poprzednią', async () => {
    let level: AutonomyLevel = 'L0';
    const execute = vi.fn(() => Promise.resolve());
    const io = {
      request: () => Promise.resolve({ status: 'opened_broker' as const, request_id: '1' }),
      readLevel: () => Promise.resolve(level),
      execute,
      notify: vi.fn(),
    };
    await acceptPlan('auto', io);
    await acceptPlan('manual', io);
    level = 'L3';
    await vi.advanceTimersByTimeAsync(4_000);
    expect(execute).not.toHaveBeenCalled();
    level = 'L1';
    await vi.advanceTimersByTimeAsync(2_000);
    expect(execute).toHaveBeenCalledTimes(1);
  });
});
