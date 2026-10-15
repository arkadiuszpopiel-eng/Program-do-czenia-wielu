import { describe, expect, it } from 'vitest';
import { appendToDraft, fillTemplate, templateNameFrom, templateVariables } from '../templates';

describe('szablony promptów', () => {
  it('zmienne w kolejności, bez powtórzeń, ze spacjami w klamrach', () => {
    expect(templateVariables('Do {{adresat}} o {{ temat }}; {{adresat}}.')).toEqual([
      'adresat',
      'temat',
    ]);
    expect(templateVariables('bez zmiennych')).toEqual([]);
  });

  it('wypełnianie zostawia puste zmienne do uzupełnienia', () => {
    expect(fillTemplate('Do {{adresat}} o {{ temat }}.', { adresat: 'Ani', temat: '' })).toBe(
      'Do Ani o {{ temat }}.',
    );
  });

  it('dopisanie do szkicu', () => {
    expect(appendToDraft('', 'X')).toBe('X');
    expect(appendToDraft('Cześć  ', 'X')).toBe('Cześć\n\nX');
  });

  it('nazwa z treści wiadomości: pierwsza linia, skrót, numer przy zajętej', () => {
    expect(templateNameFrom('  Streść   ten tekst\nw punktach', [], 'Szablon')).toBe(
      'Streść ten tekst',
    );
    const long = 'a'.repeat(50);
    expect(templateNameFrom(long, [], 'Szablon')).toBe(`${'a'.repeat(39)}…`);
    expect(templateNameFrom('Mail', ['mail', 'Mail (2)'], 'Szablon')).toBe('Mail (3)');
    expect(templateNameFrom('   ', [], 'Szablon')).toBe('Szablon');
  });
});
