import { describe, expect, it } from 'vitest';
import { SHORTCUTS } from '../shortcut-registry';
import {
  allowedInInput,
  buildKeymap,
  chordFromEvent,
  findConflicts,
  isAltGrConflict,
  normalizeChord,
  parseChord,
  type KeyLike,
  type ShortcutDef,
} from '../shortcuts';

const ev = (key: string, code: string, mods: Partial<KeyLike> = {}): KeyLike => ({
  key,
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

const def = (id: string, chord: string, scope: ShortcutDef['scope'] = 'app'): ShortcutDef => ({
  id,
  defaults: [chord],
  scope,
  group: 'view',
  customizable: true,
  inInput: false,
});

describe('skróty — parsowanie i zdarzenia', () => {
  it('kanoniczny zapis niezależnie od kolejności i wielkości liter', () => {
    expect(normalizeChord('shift+ctrl+f')).toBe('Ctrl+Shift+F');
    expect(normalizeChord('Ctrl+\\')).toBe('Ctrl+\\');
    expect(normalizeChord('Ctrl++')).toBe('Ctrl+=');
    expect(parseChord('Hyper+X')).toBeNull();
    expect(parseChord('')).toBeNull();
  });

  it('zdarzenie → skrót (litery po kodzie klawisza, interpunkcja po kodzie)', () => {
    expect(chordFromEvent(ev('k', 'KeyK', { ctrlKey: true }))).toBe('Ctrl+K');
    expect(chordFromEvent(ev('?', 'Slash', { ctrlKey: true, shiftKey: true }))).toBe(
      'Ctrl+Shift+/',
    );
    expect(chordFromEvent(ev('/', 'Slash', { ctrlKey: true }))).toBe('Ctrl+/');
    expect(chordFromEvent(ev('!', 'Digit1', { altKey: true }))).toBe('Alt+1');
    expect(chordFromEvent(ev('+', 'NumpadAdd', { ctrlKey: true }))).toBe('Ctrl+=');
    expect(chordFromEvent(ev('Escape', 'Escape'))).toBe('Esc');
    expect(chordFromEvent(ev('ArrowUp', 'ArrowUp', { ctrlKey: true }))).toBe('Ctrl+Up');
    expect(chordFromEvent(ev('Control', 'ControlLeft', { ctrlKey: true }))).toBeNull();
    expect(chordFromEvent(ev('ą', 'KeyA', { ctrlKey: true, altKey: true }))).toBe('Ctrl+Alt+A');
  });
});

describe('skróty — konflikty', () => {
  it('reguła AltGr: Ctrl+Alt(+Shift) + a c e l n o s x z', () => {
    for (const l of 'acelnosxz') expect(isAltGrConflict(`Ctrl+Alt+${l}`)).toBe(true);
    expect(isAltGrConflict('Ctrl+Alt+Shift+S')).toBe(true);
    expect(isAltGrConflict('Ctrl+Alt+Space')).toBe(false);
    expect(isAltGrConflict('Ctrl+Alt+K')).toBe(false);
  });

  it('domyślny rejestr z PLAN §14.8 nie ma konfliktów', () => {
    expect(findConflicts(SHORTCUTS)).toEqual([]);
  });

  it('duplikat po nadpisaniu użytkownika', () => {
    const conflicts = findConflicts(SHORTCUTS, { 'panel.sessions': 'Ctrl+K' });
    expect(conflicts).toEqual([
      { chord: 'Ctrl+K', ids: ['palette.open', 'panel.sessions'], reason: 'duplicate' },
    ]);
  });

  it('zarezerwowane: F5, Ctrl+R, Ctrl+Shift+F12', () => {
    const defs = [def('a', 'F5'), def('b', 'Ctrl+R'), def('c', 'Ctrl+Shift+F12')];
    expect(findConflicts(defs).map((c) => c.reason)).toEqual(['reserved', 'reserved', 'reserved']);
  });

  it('AltGr w nadpisaniu globalnego skrótu Szybkiego pytania', () => {
    const conflicts = findConflicts(SHORTCUTS, { 'system.quickAsk': 'Ctrl+Alt+S' });
    expect(conflicts).toContainEqual({
      chord: 'Ctrl+Alt+S',
      ids: ['system.quickAsk'],
      reason: 'altgr',
    });
  });

  it('mapa klawiszy pomija skróty globalne i wyłączone', () => {
    const map = buildKeymap(SHORTCUTS, { 'panel.right': '' });
    expect(map.get('Ctrl+K')?.id).toBe('palette.open');
    expect(map.get('Ctrl+Alt+Space')).toBeUndefined();
    expect(map.get('Ctrl+\\')).toBeUndefined();
  });

  it('w polu tekstowym działają tylko skróty z modyfikatorem lub F-klawisze', () => {
    const d = def('x', 'F2');
    expect(allowedInInput('F2', d)).toBe(true);
    expect(allowedInInput('Ctrl+B', d)).toBe(true);
    expect(allowedInInput('Space', d)).toBe(false);
  });
});
