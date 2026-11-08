// Język i motyw na <html> z `app_bootstrap` — dla okien pobocznych (Szybkie pytanie, pigułka)
// przed pierwszym renderem; bez tego okno zostaje w motywie systemu i z `lang` z pliku HTML.
// Moduł bez zależności: pigułka ma budżet ≤ 8 KB gzip.

/** `ui.theme`: `light`/`dark` wymusza motyw, `auto` (i brak wartości) — motyw systemu. */
export function applyTheme(root: HTMLElement, theme: unknown): void {
  if (theme === 'light' || theme === 'dark') root.setAttribute('data-theme', theme);
  else root.removeAttribute('data-theme');
}

/** Style interfejsu poza klasycznym (`ui.style`); kolory w `@alfa/ui-kit/skins.css`. */
export const SKINS = ['glass', 'next', 'junior'] as const;

/** `ui.style`: `glass` / `next` / `junior` ustawia `data-skin`, wszystko inne — styl klasyczny (bez atrybutu). */
export function applySkin(root: HTMLElement, style: unknown): void {
  if (SKINS.some((skin) => skin === style)) root.setAttribute('data-skin', style as string);
  else root.removeAttribute('data-skin');
}

/** Ustawia `lang` dokumentu oraz motyw i styl z ustawień startowych. */
export function applyBootDocument(
  root: HTMLElement,
  boot: { readonly locale: string; readonly settings: Readonly<Record<string, unknown>> },
): void {
  root.lang = boot.locale;
  applyTheme(root, boot.settings['ui.theme']);
  applySkin(root, boot.settings['ui.style']);
}
