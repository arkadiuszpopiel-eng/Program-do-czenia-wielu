import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AutonomyLevel } from '../../api/types';
import { modeItems, requestMode, stopModeWatch } from '../work-mode';

describe('tryb pracy z paska tytułu', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    stopModeWatch();
    vi.useRealTimers();
  });

  it('menu ma cztery tryby, a poziom spoza nich (L2) dopisuje w kolejności', () => {
    expect(modeItems('L3')).toEqual(['L0', 'L1', 'L3', 'L4']);
    expect(modeItems('L2')).toEqual(['L0', 'L1', 'L2', 'L3', 'L4']);
    expect(modeItems(null)).toEqual(['L0', 'L1', 'L3', 'L4']);
  });

  it('obniżenie działa od razu, bez czekania na okno Brokera', async () => {
    const notify = vi.fn();
    const readGlobal = vi.fn(() => Promise.resolve<AutonomyLevel | null>('L1'));
    await requestMode('L1', {
      request: () => Promise.resolve({ status: 'applied', request_id: '' }),
      readGlobal,
      notify,
    });
    expect(notify).toHaveBeenCalledWith('applied', 'L1');
    await vi.advanceTimersByTimeAsync(10_000);
    expect(readGlobal).not.toHaveBeenCalled();
  });

  it('podniesienie czeka na zgodę w oknie Brokera i zgłasza zapamiętany tryb raz', async () => {
    let global: AutonomyLevel = 'L3';
    const notify = vi.fn();
    const request = vi.fn(() =>
      Promise.resolve({ status: 'opened_broker' as const, request_id: '7' }),
    );
    await requestMode('L4', { request, readGlobal: () => Promise.resolve(global), notify });
    expect(request).toHaveBeenCalledWith('L4');
    expect(notify).toHaveBeenCalledWith('requested', 'L4');
    await vi.advanceTimersByTimeAsync(4_000);
    expect(notify).not.toHaveBeenCalledWith('raised', 'L4');
    global = 'L4';
    await vi.advanceTimersByTimeAsync(2_000);
    expect(notify).toHaveBeenCalledWith('raised', 'L4');
    await vi.advanceTimersByTimeAsync(10_000);
    expect(notify).toHaveBeenCalledTimes(2);
  });

  it('nowa prośba kończy czekanie na poprzednią', async () => {
    const notify = vi.fn();
    let global: AutonomyLevel = 'L3';
    const opened = () => Promise.resolve({ status: 'opened_broker' as const, request_id: '1' });
    await requestMode('L4', { request: opened, readGlobal: () => Promise.resolve(global), notify });
    await requestMode('L0', {
      request: () => Promise.resolve({ status: 'applied', request_id: '' }),
      readGlobal: () => Promise.resolve(global),
      notify,
    });
    global = 'L4';
    await vi.advanceTimersByTimeAsync(10_000);
    expect(notify).not.toHaveBeenCalledWith('raised', 'L4');
  });
});
