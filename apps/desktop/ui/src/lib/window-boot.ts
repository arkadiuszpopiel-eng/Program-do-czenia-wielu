// Język i motyw na <html> z `app_bootstrap` — dla okien pobocznych (Szybkie pytanie, pigułka)
// przed pierwszym renderem; bez tego okno zostaje w motywie systemu i z `lang` z pliku HTML.
// Moduł bez zależności: pigułka ma budżet ≤ 8 KB gzip.

/** `ui.theme`: `light`/`dark` wymusza motyw, `auto` (i brak wartości) — motyw systemu. */
export function applyTheme(root: HTMLElement, theme: unknown): void {
  if (theme === 'light' || theme === 'dark') root.setAttribute('data-theme', theme);
  else root.removeAttribute('data-theme');
}

/** Ustawia `lang` dokumentu i motyw z ustawień startowych. */
export function applyBootDocument(
  root: HTMLElement,
  boot: { readonly locale: string; readonly settings: Readonly<Record<string, unknown>> },
): void {
  root.lang = boot.locale;
  applyTheme(root, boot.settings['ui.theme']);
}
