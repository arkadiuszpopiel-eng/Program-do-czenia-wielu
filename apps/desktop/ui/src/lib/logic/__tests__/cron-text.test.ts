import { describe, expect, it } from 'vitest';
import { describeCron } from '../cron-text';

describe('opis harmonogramu cron słowami', () => {
  it('typowe harmonogramy dzienne i tygodniowe', () => {
    expect(describeCron('0 8 * * *')).toBe('codziennie o 08:00');
    expect(describeCron('0 8 * * 1-5')).toBe('w dni robocze o 08:00');
    expect(describeCron('30 21 * * 0,6')).toBe('w weekendy o 21:30');
    expect(describeCron('30 21 * * 6,7')).toBe('w weekendy o 21:30');
    expect(describeCron('15 7 * * 1,3,5')).toBe('pon., śr., pt. o 07:15');
    expect(describeCron('0 9 * * 0')).toBe('niedz. o 09:00');
    expect(describeCron('0 9 * * 0-6')).toBe('codziennie o 09:00');
  });

  it('co n minut, co n godzin, raz w miesiącu', () => {
    expect(describeCron('*/15 * * * *')).toBe('co 15 min');
    expect(describeCron('* * * * *')).toBe('co minutę');
    expect(describeCron('5 * * * *')).toBe('co godzinę, minuta 05');
    expect(describeCron('0 */2 * * *')).toBe('co 2 godz., minuta 00');
    expect(describeCron('0 6 1 * *')).toBe('1. dnia miesiąca o 06:00');
  });

  it('po angielsku', () => {
    expect(describeCron('0 8 * * 1-5', 'en')).toBe('on weekdays at 08:00');
    expect(describeCron('15 7 * * 1,3', 'en')).toBe('Mon, Wed at 07:15');
  });

  it('czego nie umie opisać pewnie — zwraca null (widok pokaże wyrażenie dosłownie)', () => {
    for (const expr of [
      '',
      '0 8 * *',
      '0 8 * 3 *',
      '0 8-10 * * *',
      '0,30 8 * * *',
      '0 8 1 * 1',
      '0 25 * * *',
      '61 8 * * *',
      '0 8 * * 9',
      '0 8 * * mon',
      '*/0 * * * *',
    ]) {
      expect(describeCron(expr), expr).toBeNull();
    }
  });
});
