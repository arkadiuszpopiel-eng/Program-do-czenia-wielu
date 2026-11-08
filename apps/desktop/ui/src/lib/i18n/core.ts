// Lekki i18n bez zależności: słowniki płaskie, interpolacja `{nazwa}`, liczby mnogie przez
// `Intl.PluralRules` (PL: one / few / many / other — „1 plik, 2 pliki, 5 plików, 1,5 pliku").

export type PluralForms = {
  readonly one?: string;
  readonly few?: string;
  readonly many?: string;
  readonly other: string;
};
export type Message = string | PluralForms;
export type Dictionary = Readonly<Record<string, Message>>;
export type Params = Readonly<Record<string, string | number>>;

const pluralRules = new Map<string, Intl.PluralRules>();
const numberFormats = new Map<string, Intl.NumberFormat>();

const tag = (locale: string): string =>
  locale === 'pl' ? 'pl-PL' : locale === 'en' ? 'en-GB' : locale;

export function pluralCategory(locale: string, n: number): Intl.LDMLPluralRule {
  let rules = pluralRules.get(locale);
  if (!rules) {
    rules = new Intl.PluralRules(tag(locale));
    pluralRules.set(locale, rules);
  }
  return rules.select(n);
}

export function formatNumber(locale: string, n: number): string {
  let format = numberFormats.get(locale);
  if (!format) {
    format = new Intl.NumberFormat(tag(locale), { maximumFractionDigits: 2 });
    numberFormats.set(locale, format);
  }
  return format.format(n);
}

export function pickPlural(forms: PluralForms, locale: string, n: number): string {
  const category = pluralCategory(locale, n);
  switch (category) {
    case 'one':
      return forms.one ?? forms.other;
    case 'few':
      return forms.few ?? forms.other;
    case 'many':
      return forms.many ?? forms.other;
    default:
      return forms.other;
  }
}

export function interpolate(template: string, locale: string, params?: Params): string {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const value = params[name];
    if (value === undefined) return whole;
    return typeof value === 'number' ? formatNumber(locale, value) : value;
  });
}

/**
 * Tłumaczenie klucza. Dla form mnogich liczba pochodzi z parametru `n`.
 * Brak klucza w słowniku języka → słownik zapasowy (PL) → sam klucz (widoczny w testach).
 */
export function translate(
  dict: Dictionary,
  fallback: Dictionary,
  locale: string,
  key: string,
  params?: Params,
): string {
  const message = dict[key] ?? fallback[key];
  if (message === undefined) return key;
  if (typeof message === 'string') return interpolate(message, locale, params);
  const n = typeof params?.['n'] === 'number' ? params['n'] : 0;
  return interpolate(pickPlural(message, locale, n), locale, params);
}
