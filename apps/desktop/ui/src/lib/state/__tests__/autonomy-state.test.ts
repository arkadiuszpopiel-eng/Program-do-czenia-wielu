import { describe, expect, it, vi } from 'vitest';
import type { PermissionsApi } from '../../api/client';
import type { ChildLockState, PermissionsState } from '../../api/types-hub';
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

  it('kłódka otwarta na czas: po terminie stan odczytany od nowa (plan 50 §4.4)', async () => {
    vi.useFakeTimers();
    try {
      const now = Date.now();
      const states: ChildLockState[] = [
        {
          pin_set: true,
          enabled: true,
          open: true,
          open_until_ms: now + 1_000,
          attempts_left: 5,
          tamper_resistant: false,
          unknown: false,
          transcripts: false,
          conversation: false,
        },
        {
          pin_set: true,
          enabled: true,
          open: false,
          attempts_left: 5,
          tamper_resistant: false,
          unknown: false,
          transcripts: false,
          conversation: false,
        },
      ];
      let reads = 0;
      const client = {
        get: () => Promise.resolve(reply({ global: 'L3' })),
        childLock: () => Promise.resolve(states[Math.min(reads++, 1)]),
      } as unknown as PermissionsApi;
      const autonomy = new AutonomyState();
      await autonomy.load(client);
      expect(autonomy.locked).toBe(false);
      await vi.advanceTimersByTimeAsync(1_600);
      expect(reads).toBe(2);
      expect(autonomy.locked).toBe(true);
    } finally {
      vi.useRealTimers();
    }
  });

  it('stan profilu nieznany (zerwane łącze) — zostaje ostatni znany stan kłódki', async () => {
    const base = {
      pin_set: true,
      open: false,
      attempts_left: 5,
      tamper_resistant: false,
      transcripts: false,
      conversation: false,
    };
    const states: ChildLockState[] = [
      { ...base, enabled: true, unknown: false },
      { ...base, pin_set: false, enabled: false, unknown: true },
    ];
    let reads = 0;
    const client = {
      get: () => Promise.resolve(reply({ global: reads === 0 ? 'L1' : 'L0' })),
      childLock: () => Promise.resolve(states[Math.min(reads++, 1)]),
    } as unknown as PermissionsApi;
    const autonomy = new AutonomyState();
    await autonomy.load(client);
    expect(autonomy.locked).toBe(true);
    // Odpowiedź zastępcza bez łącza (L0) nie zmienia pokazywanego trybu.
    expect(await autonomy.load(client)).toBe('L1');
    expect(autonomy.global).toBe('L1');
    expect(autonomy.locked).toBe(true);
    // Bez wcześniejszego odczytu — nieznany stan nie udaje kłódki.
    const fresh = new AutonomyState();
    const lost = {
      get: () => Promise.resolve(reply({ global: 'L1' })),
      childLock: () => Promise.resolve(states[1]),
    } as unknown as PermissionsApi;
    await fresh.load(lost);
    expect(fresh.locked).toBe(false);
  });

  it('błąd odczytu profilu nie zdejmuje znaku „Rodzic czyta”; świeży widok z karty wprost', async () => {
    const reading: ChildLockState = {
      pin_set: true,
      enabled: true,
      open: false,
      attempts_left: 5,
      tamper_resistant: false,
      unknown: false,
      transcripts: true,
      conversation: false,
    };
    let fail = false;
    const client = {
      get: () => Promise.resolve(reply({ global: 'L1' })),
      childLock: () => (fail ? Promise.reject(new Error('łącze')) : Promise.resolve(reading)),
    } as unknown as PermissionsApi;
    const autonomy = new AutonomyState();
    await autonomy.load(client);
    expect(autonomy.child?.transcripts).toBe(true);
    fail = true;
    await autonomy.load(client);
    expect(autonomy.child?.transcripts).toBe(true);
    autonomy.applyChild({ ...reading, transcripts: false });
    expect(autonomy.child?.transcripts).toBe(false);
    autonomy.applyChild({ ...reading, unknown: true, enabled: false });
    expect(autonomy.child?.enabled).toBe(true);
  });
});
