import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AutonomyLevel } from '../../api/types';
import type { AppState } from '../../state/app.svelte';
import { modeView, nextMode, switchMode } from '../../state/mode-switch';
import { stopModeWatch } from '../work-mode';

function app(level: AutonomyLevel = 'L1', locked = false) {
  const requestLevel = vi.fn(() => Promise.resolve({ status: 'applied', request_id: '' }));
  const state = {
    sessions: { active: { id: 'session-a', autonomy: level } },
    autonomy: { global: 'L3', locked, openHere: () => false, load: () => Promise.resolve('L3') },
    client: {
      permissions: { requestLevel, get: () => Promise.resolve({ session: level, global: 'L3' }) },
    },
    i18n: { t: (key: string) => key, tk: (key: string) => key },
    toasts: { show: vi.fn() },
  } as unknown as AppState;
  return { state, requestLevel };
}

afterEach(stopModeWatch);

describe('zakres pięciu trybów pracy', () => {
  it('skrót bez rozmowy pomija Plan i wraca z Automatycznie do Ręcznie', () => {
    const { state } = app();
    Object.assign(state.sessions, { active: null });
    const view = modeView(state);
    expect(nextMode(state, { ...view, current: 'L1' })).toBe('L2');
    expect(nextMode(state, { ...view, current: 'L2' })).toBe('L3');
    expect(nextMode(state, { ...view, current: 'L3' })).toBe('L1');
  });
  it('Plan bez aktywnej rozmowy nie zmienia poziomu globalnego', async () => {
    const { state, requestLevel } = app();
    Object.assign(state.sessions, { active: null });
    await switchMode(state, modeView(state), 'L0');
    expect(requestLevel).not.toHaveBeenCalled();
    expect(state.toasts.show).toHaveBeenCalledWith({ kind: 'info', message: 'mode.needsSession' });
  });

  it.each<AutonomyLevel>(['L1', 'L2', 'L4'])(
    '%s bez aktywnej rozmowy nadal można wybrać globalnie',
    async (level) => {
      const { state, requestLevel } = app();
      Object.assign(state.sessions, { active: null });
      await switchMode(state, modeView(state), level);
      expect(requestLevel).toHaveBeenCalledExactlyOnceWith(level, null);
    },
  );

  it('osobny wybór rozmowy pokazuje jej poziom, a globalny poziom całego programu', () => {
    const { state } = app('L2');
    expect(modeView(state, 'session').current).toBe('L2');
    expect(modeView(state, 'session').scoped).toBe(true);
    expect(modeView(state, 'global').current).toBe('L3');
    expect(modeView(state, 'global').scoped).toBe(false);
  });

  it.each<AutonomyLevel>(['L0', 'L1', 'L2', 'L3', 'L4'])(
    '%s można wybrać dla jednej rozmowy',
    async (level) => {
      const { state, requestLevel } = app(level === 'L1' ? 'L2' : 'L1');
      await switchMode(state, modeView(state, 'session'), level);
      expect(requestLevel).toHaveBeenCalledExactlyOnceWith(level, 'session-a');
    },
  );

  it.each<AutonomyLevel>(['L1', 'L2', 'L4'])(
    '%s w zakresie globalnym zachowuje jawny zakres',
    async (level) => {
      const { state, requestLevel } = app();
      await switchMode(state, modeView(state, 'global'), level);
      expect(requestLevel).toHaveBeenCalledExactlyOnceWith(level, null);
    },
  );

  it('Plan i wyjście z Planu zawsze dotyczą rozmowy', async () => {
    const { state, requestLevel } = app('L0');
    await switchMode(state, modeView(state, 'global'), 'L2');
    expect(requestLevel).toHaveBeenCalledExactlyOnceWith('L2', 'session-a');
  });

  it.each<AutonomyLevel>(['L2', 'L3', 'L4'])(
    'kłódka rodzica nadal blokuje %s w zakresie rozmowy',
    async (level) => {
      const { state, requestLevel } = app('L1', true);
      await switchMode(state, modeView(state, 'session'), level);
      expect(requestLevel).not.toHaveBeenCalled();
    },
  );
});
