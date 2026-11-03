import { describe, expect, it } from 'vitest';
import type { PermissionsApi } from '../../api/client';
import type { PermissionsState } from '../../api/types-hub';
import { AutonomyState } from '../autonomy.svelte';

const reply = (state: Partial<PermissionsState>): PermissionsState => ({
  global: 'L3',
  session: null,
  hello_enabled: false,
  ...state,
});

const api = (get: PermissionsApi['get']): PermissionsApi => ({ get }) as unknown as PermissionsApi;

describe('AutonomyState — tryb pracy i „Max dostęp” (plan 50 §8.1)', () => {
  it('odczyt ustawia poziom globalny i zasięg Max dostępu; brak pola = wyłączony', async () => {
    const autonomy = new AutonomyState();
    await autonomy.load(api(() => Promise.resolve(reply({ global: 'L4', max_access: 'cloud' }))));
    expect(autonomy.global).toBe('L4');
    expect(autonomy.maxAccess).toBe('cloud');
    await autonomy.load(api(() => Promise.resolve(reply({}))));
    expect(autonomy.maxAccess).toBeNull();
  });

  it('spóźniony odczyt nie nadpisuje nowszego', async () => {
    const autonomy = new AutonomyState();
    let resolve: (v: PermissionsState) => void = () => {};
    const slow = autonomy.load(api(() => new Promise((r) => (resolve = r))));
    await autonomy.load(api(() => Promise.resolve(reply({ max_access: 'local' }))));
    resolve(reply({ max_access: 'all' }));
    await slow;
    expect(autonomy.maxAccess).toBe('local');
  });

  it('nieudany odczyt zostawia stan i zwraca null', async () => {
    const autonomy = new AutonomyState();
    await autonomy.load(api(() => Promise.resolve(reply({ max_access: 'all' }))));
    const result = await autonomy.load(api(() => Promise.reject(new Error('brak Sędziego'))));
    expect(result).toBeNull();
    expect(autonomy.maxAccess).toBe('all');
  });
});
