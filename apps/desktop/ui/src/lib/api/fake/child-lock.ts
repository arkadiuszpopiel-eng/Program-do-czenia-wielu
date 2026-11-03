// Atrapa profilu Dziecko (plan 50 §4.4) — zachowuje się jak Sędzia Cieni: PIN 4–8 cyfr, 5 błędnych
// prób i blokada 15 min, kłódka obniża tryb do „Ręcznie” i odrzuca podniesienia. Stan startowy
// z adresu: `?child=locked` (PIN atrapy 2468), `open` (otwarta do zamknięcia Alfy), `off` (PIN
// ustawiony, profil wyłączony); bez parametru — brak PIN-u.
import { CommandError } from '../command-error';
import type { AutonomyLevel } from '../types';
import type { ChildLockRequest, ChildLockState } from '../types-hub';
import type { FakeCore } from './core';

/** PIN rodzica w atrapie (testy E2E). */
export const FAKE_PIN = '2468';
const ATTEMPTS = 5;
const BLOCK_MS = 15 * 60_000;

export interface FakeChildLock {
  childLock(change: ChildLockRequest | null): Promise<ChildLockState>;
  /** Kłódka zamknięta teraz — Sędzia ogranicza tryb do „Ręcznie”. */
  locked(): boolean;
  /** Poziom po kłódce. */
  cap(level: AutonomyLevel): AutonomyLevel;
}

export function childLockFake(core: FakeCore): FakeChildLock {
  const mode = new URLSearchParams(globalThis.location?.search ?? '').get('child');
  let pin: string | null = mode === 'locked' || mode === 'open' || mode === 'off' ? FAKE_PIN : null;
  let enabled = mode === 'locked' || mode === 'open';
  /** Do kiedy otwarta; `Infinity` — do zamknięcia Alfy, `null` — zamknięta. */
  let openUntil: number | null = mode === 'open' ? Infinity : null;
  let failures = 0;
  let blockedUntil = 0;

  const isOpen = () => enabled && openUntil !== null && Date.now() < openUntil;
  const state = (): ChildLockState => ({
    pin_set: pin !== null,
    enabled,
    open: isOpen(),
    open_until_ms: isOpen() && openUntil !== Infinity ? openUntil : null,
    blocked_until_ms: Date.now() < blockedUntil ? blockedUntil : null,
    attempts_left: ATTEMPTS - failures,
  });
  const refuse = (message: string) =>
    Promise.reject(new CommandError('invalid_input', `Sędzia Cieni: ${message}`));
  /** Sprawdzenie PIN-u jak w Sędzim; `null` — poprawny. */
  const check = (given: string): string | null => {
    if (Date.now() < blockedUntil) return 'za dużo błędnych prób — PIN chwilowo zablokowany';
    if (pin === null) return 'PIN rodzica nie jest ustawiony';
    if (given !== pin) {
      failures += 1;
      if (failures >= ATTEMPTS) {
        failures = 0;
        blockedUntil = Date.now() + BLOCK_MS;
      }
      return 'błędny PIN rodzica';
    }
    failures = 0;
    return null;
  };
  const valid = (p: string) => /^\d{4,8}$/.test(p);

  function childLock(change: ChildLockRequest | null): Promise<ChildLockState> {
    if (change === null) return core.reply(state());
    const pins = [
      'pin' in change ? change.pin : null,
      'new' in change ? change.new : null,
      'old' in change ? change.old : null,
    ];
    if (pins.some((p) => p !== null && !valid(p))) {
      return Promise.reject(new CommandError('invalid_input', 'PIN ma od 4 do 8 cyfr'));
    }
    let error: string | null = null;
    switch (change.op) {
      case 'close':
        openUntil = null;
        break;
      case 'set_pin':
        if (pin !== null)
          error = change.old === null ? 'zmiana PIN-u wymaga starego PIN-u' : check(change.old);
        if (error === null) pin = change.new;
        break;
      case 'enable':
        error = check(change.pin);
        if (error === null) {
          enabled = change.on;
          openUntil = null;
        }
        break;
      case 'verify':
        error = check(change.pin);
        break;
      case 'open':
        if (!enabled) error = 'profil Dziecko jest wyłączony';
        else error = check(change.pin);
        if (error === null) {
          openUntil = change.minutes === null ? Infinity : Date.now() + change.minutes * 60_000;
        }
        break;
    }
    return error === null ? core.reply(state()) : refuse(error);
  }

  const locked = () => enabled && !isOpen();
  return {
    childLock,
    locked,
    cap: (level) => (locked() && level > 'L1' ? 'L1' : level),
  };
}
