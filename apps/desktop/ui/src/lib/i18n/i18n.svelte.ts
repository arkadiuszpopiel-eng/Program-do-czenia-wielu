// Stan języka (runy): `t()` czyta `locale`, więc szablony przerysowują się po zmianie języka.
// Słownik EN jest ładowany leniwie — nie obciąża paczki startowej.
import type { Locale, LocalizedText, Money } from '../api/types';
import { translate, type Dictionary, type Params } from './core';
import * as fmt from './format';
import { pl, type MessageKey } from './pl';

export type { MessageKey } from './pl';

export class I18n {
  locale = $state<Locale>('pl');
  #dicts = $state.raw<Partial<Record<Locale, Dictionary>>>({ pl });

  readonly t = (key: MessageKey, params?: Params): string =>
    translate(this.#dicts[this.locale] ?? pl, pl, this.locale, key, params);

  /** Klucz budowany dynamicznie (np. `role.${id}`); brak klucza → sam klucz. */
  readonly tk = (key: string, params?: Params): string =>
    translate(this.#dicts[this.locale] ?? pl, pl, this.locale, key, params);

  readonly text = (value: LocalizedText): string => value[this.locale];
  readonly money = (value: Money): string => fmt.formatMoney(this.locale, value);
  readonly int = (value: number): string => fmt.formatInteger(this.locale, value);
  readonly percent = (ratio: number): string => fmt.formatPercent(this.locale, ratio);
  readonly bytes = (value: number): string => fmt.formatBytes(this.locale, value);
  readonly duration = (ms: number): string => fmt.formatDuration(this.locale, ms);
  readonly time = (iso: string): string => fmt.formatTime(this.locale, iso);
  readonly dateTime = (iso: string): string => fmt.formatDateTime(this.locale, iso);
  readonly day = (iso: string): string => fmt.formatDay(this.locale, iso);
  readonly relative = (iso: string, now?: number): string =>
    fmt.formatRelative(this.locale, iso, now);

  async setLocale(locale: Locale): Promise<void> {
    if (!this.#dicts[locale]) {
      const { en } = await import('./en');
      this.#dicts = { ...this.#dicts, en };
    }
    this.locale = locale;
    if (typeof document !== 'undefined') document.documentElement.lang = locale;
  }
}

/** Jedna instancja na okno. */
export const i18n = new I18n();
