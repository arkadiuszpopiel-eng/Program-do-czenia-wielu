// Gotowe obowiązki codzienne agentek (plan P14): szablon = nazwa, harmonogram, przypisana agentka
// i polecenie. Wybranie szablonu tylko wypełnia formularz wyzwalacza — użytkownik może wszystko
// zmienić przed utworzeniem. Są tu wyłącznie obowiązki, które działają na tym, co Alfa już ma
// (zadania, koszty, katalog roboczy, rozmowy); poczta i kalendarz dojdą z łącznikami.
import type { MessageKey } from '../i18n/i18n.svelte';

export interface DutyTemplate {
  readonly id: string;
  /** Klucz nazwy (także nazwa utworzonego wyzwalacza). */
  readonly name: MessageKey;
  /** Klucz polecenia dla agentki. */
  readonly goal: MessageKey;
  /** Klucz krótkiego opisu harmonogramu („dni robocze o 8:00”). */
  readonly when: MessageKey;
  /** Harmonogram cron (Europe/Warsaw). */
  readonly cron: string;
  /** Identyfikator agentki. */
  readonly agent: 'alfa' | 'beta' | 'gama' | 'delta';
}

export const DUTY_TEMPLATES: readonly DutyTemplate[] = [
  {
    id: 'morning',
    name: 'duty.morning.name',
    goal: 'duty.morning.goal',
    when: 'duty.morning.when',
    cron: '0 8 * * 1-5',
    agent: 'alfa',
  },
  {
    id: 'evening',
    name: 'duty.evening.name',
    goal: 'duty.evening.goal',
    when: 'duty.evening.when',
    cron: '0 18 * * 1-5',
    agent: 'beta',
  },
  {
    id: 'tidy',
    name: 'duty.tidy.name',
    goal: 'duty.tidy.goal',
    when: 'duty.tidy.when',
    cron: '0 12 * * 6',
    agent: 'gama',
  },
  {
    id: 'costs',
    name: 'duty.costs.name',
    goal: 'duty.costs.goal',
    when: 'duty.costs.when',
    cron: '0 9 * * 1',
    agent: 'delta',
  },
];

/** Pięć pól cron z dozwolonymi znakami (wstępna kontrola; dokładną robi rdzeń w podglądzie). */
export function looksLikeCron(expr: string): boolean {
  const fields = expr.trim().split(/\s+/);
  return fields.length === 5 && fields.every((f) => /^[\d*,/-]+$/.test(f));
}
