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

  it('szczegóły błędu są dostępne dla widoku, a dziwne wartości pomijane', () => {
    const e = toError({
      code: 'provider',
      message: 'Limit zapytań.',
      details: { provider: 'anthropic', reason: 'rate_limited', retry_after_s: 30, list: [1] },
    }) as CommandError;
    expect(e.detail('provider')).toBe('anthropic');
    expect(e.detail('reason')).toBe('rate_limited');
    expect(e.retryAfterSeconds).toBe(30);
    expect(e.detail('retry_after_s')).toBeNull();
    expect('list' in e.details).toBe(false);
    const plain = toError({ code: 'not_found', message: 'Nie ma.' }) as CommandError;
    expect(plain.details).toEqual({});
    expect(plain.retryAfterSeconds).toBeNull();
    expect((toError({ message: 'm', details: 'tekst' }) as CommandError).details).toEqual({});
  });

  it('obiekt bez kodu dostaje kod internal', () => {
    expect((toError({ message: 'm' }) as CommandError).code).toBe('internal');
  });
});
