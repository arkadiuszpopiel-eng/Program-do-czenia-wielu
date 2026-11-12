import { describe, expect, it, vi } from 'vitest';
import { setBuilderRole } from './builder-assignment';
import type { AlfaClient } from '../api/client';
import type { AgentState } from '../api/types';

describe('przypisanie agentki z biblioteki', () => {
  function client() {
    let states: AgentState[] = [
      { id: 'luna', role_ids: ['inna-rola'], status: 'idle', activity: null },
      { id: 'alfa', role_ids: ['coordinator'], status: 'idle', activity: null },
    ];
    const agents = {
      list: vi.fn(async () => states),
      setRoles: vi.fn(async (_sid: string, id: string, roles: string[]) => {
        states = states.map((a) => (a.id === id ? { ...a, role_ids: roles } : a));
      }),
    };
    return { agents } as Pick<AlfaClient, 'agents'> & { agents: typeof agents };
  }

  it('czyta aktualne role i po dodaniu/odłączeniu zachowuje pozostałe role i persony', async () => {
    const c = client();
    await setBuilderRole(c, 's-test', 'luna', 'czytelniczka', true);
    expect(c.agents.setRoles).toHaveBeenLastCalledWith('s-test', 'luna', [
      'inna-rola',
      'czytelniczka',
    ]);
    // Zmiana obsady przez inne istniejące sterowanie między dwoma kliknięciami.
    await c.agents.setRoles('s-test', 'luna', ['inna-rola', 'czytelniczka', 'nowa-rola']);
    const after = await setBuilderRole(c, 's-test', 'luna', 'czytelniczka', false);
    expect(c.agents.setRoles).toHaveBeenLastCalledWith('s-test', 'luna', [
      'inna-rola',
      'nowa-rola',
    ]);
    expect(after.find((a) => a.id === 'alfa')?.role_ids).toEqual(['coordinator']);
  });

  it('nie zapisuje ról po nieudanym odczycie i zwraca błąd rdzenia', async () => {
    const c = client();
    c.agents.list.mockRejectedValueOnce(new Error('brak rozmowy'));
    await expect(setBuilderRole(c, 's-test', 'luna', 'czytelniczka', true)).rejects.toThrow(
      'brak rozmowy',
    );
    expect(c.agents.setRoles).not.toHaveBeenCalled();
    c.agents.setRoles.mockRejectedValueOnce(new Error('odmowa'));
    await expect(setBuilderRole(c, 's-test', 'luna', 'czytelniczka', true)).rejects.toThrow(
      'odmowa',
    );
    expect((await c.agents.list('s-test'))[0]?.role_ids).toEqual(['inna-rola']);
  });
});
