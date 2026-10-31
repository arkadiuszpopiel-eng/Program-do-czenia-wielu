import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AutonomyLevel } from '../../api/types';
import { watchRaise } from '../raise-watch';

describe('czekanie na podniesienie zatwierdzone w oknie Sędziego Cieni', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('zgłasza zmianę, gdy Sędzia Cieni przyjmie poziom z prośby, i przestaje pytać', async () => {
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

  it('odczyt w locie po przerwaniu niczego nie zgłasza (nowa prośba, opuszczenie strony)', async () => {
    let answer: (level: AutonomyLevel | null) => void = () => undefined;
    const read = vi.fn(
      () =>
        new Promise<AutonomyLevel | null>((resolve) => {
          answer = resolve;
        }),
    );
    const onApplied = vi.fn();
    const stop = watchRaise({ level: 'L4', read, onApplied, everyMs: 100 });
    await vi.advanceTimersByTimeAsync(100);
    expect(read).toHaveBeenCalledTimes(1);
    stop();
    answer('L4');
    await vi.advanceTimersByTimeAsync(500);
    expect(onApplied).not.toHaveBeenCalled();
    expect(read).toHaveBeenCalledTimes(1);
  });
});
