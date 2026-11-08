import { describe, expect, it } from 'vitest';
import { en } from '../../i18n/en';
import { pl } from '../../i18n/pl';
import { DUTIES_ENABLED, DUTY_TEMPLATES, looksLikeCron } from '../duties';

describe('gotowe obowiązki codzienne', () => {
  it('wzory są pokazywane, a każde polecenie odwołuje się do danych z programu', () => {
    // Rdzeń dokleja arkusz faktów do zadania w tle; polecenie bez odwołania do niego zachęca do zmyślania.
    expect(DUTIES_ENABLED).toBe(true);
    // „Kontrola cenników” czyta strony dostawców (narzędzia sieci), nie arkusz faktów — jej polecenie
    // zabrania zgadywania i zapisu plików, a wynik zatwierdza właściciel w Ustawieniach.
    for (const duty of DUTY_TEMPLATES.filter((d) => d.id !== 'prices')) {
      expect(String(pl[duty.goal]), duty.id).toMatch(/dan(ych|e) z programu/);
      expect(String(en[duty.goal]), duty.id).toMatch(/program data/);
    }
  });

  it('kontrola cenników: tylko ceny ze stron, bez zgadywania i zapisu, wynik do zatwierdzenia', () => {
    const goal = String(pl['duty.prices.goal']);
    expect(goal).toMatch(/niczego nie zgaduj/);
    expect(goal).toMatch(/Nie zapisuj ani nie zmieniaj żadnych plików/);
    expect(goal).toMatch(/\[\[price\]\]/);
    expect(goal).toMatch(/https:\/\/developers\.openai\.com/);
    expect(String(en['duty.prices.goal'])).toMatch(/do not guess/);
  });

  it('każdy szablon ma harmonogram, agentkę i teksty w obu językach', () => {
    expect(DUTY_TEMPLATES.length).toBeGreaterThanOrEqual(4);
    const ids = new Set<string>();
    for (const duty of DUTY_TEMPLATES) {
      expect(ids.has(duty.id), duty.id).toBe(false);
      ids.add(duty.id);
      expect(looksLikeCron(duty.cron), duty.cron).toBe(true);
      expect(['alfa', 'beta', 'gama', 'delta']).toContain(duty.agent);
      for (const key of [duty.name, duty.goal, duty.when]) {
        expect(pl[key], key).toBeTruthy();
        expect(en[key], key).toBeTruthy();
      }
      // Polecenie ma być pełnym zdaniem, a nie hasłem.
      expect(String(pl[duty.goal]).length).toBeGreaterThan(60);
    }
  });

  it('porządki nigdy nie każą niczego usuwać bez zgody', () => {
    const tidy = DUTY_TEMPLATES.find((d) => d.id === 'tidy');
    const goal = tidy ? String(pl[tidy.goal]) : '';
    expect(goal).toMatch(/Niczego nie usuwaj/);
    expect(goal).toMatch(/poczekaj na moją zgodę/);
  });

  it('wstępna kontrola harmonogramu odrzuca oczywiste błędy', () => {
    expect(looksLikeCron('0 8 * * 1-5')).toBe(true);
    expect(looksLikeCron('*/15 * * * *')).toBe(true);
    expect(looksLikeCron('0 8 * *')).toBe(false);
    expect(looksLikeCron('codziennie o ósmej')).toBe(false);
    expect(looksLikeCron('')).toBe(false);
  });
});
