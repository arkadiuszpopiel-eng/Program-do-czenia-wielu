import { describe, expect, it } from 'vitest';
import { appendToDraft, fillTemplate, templateVariables } from '../templates';

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
});
