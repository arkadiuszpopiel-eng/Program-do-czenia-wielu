import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AutonomyLevel } from '../../api/types';
import { watchRaise } from '../raise-watch';

describe('czekanie na podniesienie zatwierdzone w oknie Brokera', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('zgłasza zmianę, gdy Broker przyjmie poziom z prośby, i przestaje pytać', async () => {
    let level: AutonomyLevel = 'L3';
    const read = vi.fn(() => Promise.resolve<AutonomyLevel | null>(level));
    const onApplied = vi.fn();
    watchRaise({ level: 'L4', read, onApplied, everyMs: 100, forMs: 10_000 });
    await vi.advanceTimersByTimeAsync(250);
    expect(onApplied).not.toHaveBeenCalled();
    // Właściciel zatwierdza w oknie Brokera.
    level = 'L4';
    await vi.advanceTimersByTimeAsync(100);
    expect(onApplied).toHaveBeenCalledTimes(1);
    const calls = read.mock.calls.length;
    await vi.advanceTimersByTimeAsync(1_000);
    expect(read.mock.calls.length).toBe(calls);
    expect(onApplied).toHaveBeenCalledTimes(1);
  });

  it('kończy po czasie życia karty, przeżywa błędy odczytu i daje się przerwać', async () => {
    const read = vi.fn(() => Promise.reject(new Error('łącze')));
    const onApplied = vi.fn();
    watchRaise({ level: 'L4', read, onApplied, everyMs: 100, forMs: 500 });
    await vi.advanceTimersByTimeAsync(2_000);
    expect(read).toHaveBeenCalledTimes(5);
    expect(onApplied).not.toHaveBeenCalled();

    const later = vi.fn(() => Promise.resolve<AutonomyLevel | null>('L3'));
    const stop = watchRaise({ level: 'L4', read: later, onApplied, everyMs: 100 });
    await vi.advanceTimersByTimeAsync(200);
    stop();
    const calls = later.mock.calls.length;
    await vi.advanceTimersByTimeAsync(1_000);
    expect(later.mock.calls.length).toBe(calls);
  });
});
