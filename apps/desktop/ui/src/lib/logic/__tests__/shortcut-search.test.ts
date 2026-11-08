import { describe, expect, it } from 'vitest';
import { foldText, matchesShortcut } from '../shortcut-search';

describe('wyszukiwanie w ściągawce skrótów', () => {
  it('ignoruje wielkość liter, polskie znaki i plusy', () => {
    expect(foldText('  Zmień  Język+Ł ')).toBe('zmien jezyk l');
    expect(foldText('Ctrl+Shift+M')).toBe('ctrl shift m');
  });

  it('szuka po nazwie polecenia', () => {
    expect(matchesShortcut('wstecz', 'Wstecz (poprzednie miejsce)', ['Alt+Left'])).toBe(true);
    expect(matchesShortcut('MIKROFON', 'Mikrofon wł./wył.', ['Ctrl+Shift+M'])).toBe(true);
    expect(matchesShortcut('pamiec', 'Panel: Pamięć', ['Alt+4'])).toBe(true);
    expect(matchesShortcut('kamera', 'Panel: Pamięć', ['Alt+4'])).toBe(false);
  });

  it('szuka po klawiszach i łączy słowa', () => {
    expect(matchesShortcut('alt+1', 'Panel: Agentki', ['Alt+1'])).toBe(true);
    expect(matchesShortcut('ctrl k', 'Paleta poleceń', ['Ctrl+K'])).toBe(true);
    expect(matchesShortcut('panel alt', 'Panel: Agentki', ['Alt+1'])).toBe(true);
    expect(matchesShortcut('panel ctrl', 'Panel: Agentki', ['Alt+1'])).toBe(false);
  });

  it('puste zapytanie pokazuje wszystko', () => {
    expect(matchesShortcut('', 'Cokolwiek', [])).toBe(true);
    expect(matchesShortcut('   ', 'Cokolwiek', [])).toBe(true);
  });
});
