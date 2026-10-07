import { describe, expect, it } from 'vitest';
import { CommandError, errorText, toError } from '../command-error';

describe('toError', () => {
  it('AppError z rdzenia (zwykły obiekt) staje się Error z komunikatem i kodem', () => {
    const e = toError({ code: 'invalid_input', message: 'katalog nie zna endpointu' });
    expect(e).toBeInstanceOf(CommandError);
    expect(e.message).toBe('katalog nie zna endpointu');
    expect((e as CommandError).code).toBe('invalid_input');
  });

  it('nie pokazuje „[object Object]"', () => {
    expect(errorText({ code: 'internal', message: 'x' })).toBe('x');
    expect(errorText({ unexpected: true })).not.toBe('');
  });

  it('Error zostaje bez zmian, napis staje się komunikatem', () => {
    const original = new Error('a');
    expect(toError(original)).toBe(original);
    expect(errorText('brak sieci')).toBe('brak sieci');
  });

  it('obiekt bez kodu dostaje kod internal', () => {
    expect((toError({ message: 'm' }) as CommandError).code).toBe('internal');
  });
});
