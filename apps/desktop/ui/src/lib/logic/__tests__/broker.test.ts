import { describe, expect, it } from 'vitest';
import type { BrokerStatusView } from '../../api/types-broker';
import { bannerIsSticky, brokerBanner, safeState } from '../broker';

const service: BrokerStatusView = {
  mode: 'service',
  state: 'connected',
  approval_window: true,
  watchdog: true,
  isolated: true,
  detail: null,
};

describe('baner Brokera', () => {
  it('usługa z watchdogiem — bez banera i bez bezpiecznego stanu', () => {
    expect(brokerBanner(service)).toBeNull();
    expect(safeState(service)).toBe(false);
    expect(brokerBanner(null)).toBeNull();
    expect(safeState(null)).toBe(false);
  });

  it('zerwanie i brak Brokera to bezpieczny stan, którego nie da się ukryć', () => {
    const lost = { ...service, state: 'lost' as const, approval_window: false };
    expect(brokerBanner(lost, ['lost'])).toBe('lost');
    expect(safeState(lost)).toBe(true);
    expect(bannerIsSticky('lost')).toBe(true);
    const none = { ...service, mode: 'unavailable' as const, state: 'lost' as const };
    expect(brokerBanner(none)).toBe('unavailable');
    expect(bannerIsSticky('unavailable')).toBe(true);
    const connecting = { ...service, state: 'connecting' as const };
    expect(brokerBanner(connecting)).toBe('connecting');
    expect(safeState(connecting)).toBe(true);
  });

  it('watchdog i tryb deweloperski — banery informacyjne do ukrycia', () => {
    const noWatchdog = { ...service, watchdog: false };
    expect(brokerBanner(noWatchdog)).toBe('watchdog');
    expect(brokerBanner(noWatchdog, ['watchdog'])).toBeNull();
    expect(bannerIsSticky('watchdog')).toBe(false);
    const dev = { ...service, mode: 'in_process' as const, watchdog: false, isolated: false };
    expect(brokerBanner(dev)).toBe('dev');
    expect(brokerBanner(dev, ['dev'])).toBeNull();
  });
});
