// Wspólne narzędzia przeglądu układu (`review-sweep*.spec.ts`): wady układu widoczne na ekranie
// i błędy konsoli.
import type { Page } from '@playwright/test';

export const STYLES = ['classic', 'glass', 'next'] as const;
export const THEMES = ['light', 'dark'] as const;

/** Wady układu widoczne na ekranie: opis elementu i powód. */
export async function layoutDefects(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const defects: string[] = [];
    const width = window.innerWidth;
    const describe = (el: Element): string => {
      const text = (el.textContent ?? '').trim().replace(/\s+/g, ' ').slice(0, 40);
      const cls = typeof el.className === 'string' ? el.className.split(' ')[0] : '';
      return `<${el.tagName.toLowerCase()}${cls ? `.${cls}` : ''}> „${text}”`;
    };
    if (document.documentElement.scrollWidth > width + 1)
      defects.push(
        `strona przewija się w poziomie (${document.documentElement.scrollWidth} > ${width})`,
      );
    const visible = (el: Element): boolean => {
      const style = getComputedStyle(el);
      if (style.visibility === 'hidden' || style.display === 'none' || style.opacity === '0')
        return false;
      const rect = el.getBoundingClientRect();
      // Tekst tylko dla czytników ekranu (1 × 1 px) nie jest częścią układu.
      if (rect.width <= 2 || rect.height <= 2) return false;
      return rect.bottom > 0 && rect.top < window.innerHeight;
    };
    /** Element w przewijanym w poziomie albo przycinającym przodku ma prawo wystawać. */
    const clippedByAncestor = (el: Element): boolean => {
      for (let node = el.parentElement; node; node = node.parentElement) {
        const overflow = getComputedStyle(node).overflowX;
        if (overflow !== 'visible') return true;
      }
      return false;
    };
    for (const el of document.querySelectorAll(
      'button, a, input, [role="combobox"], h1, h2, h3, label',
    )) {
      if (!visible(el)) continue;
      const rect = el.getBoundingClientRect();
      if (rect.right > width + 1 && !clippedByAncestor(el))
        defects.push(`${describe(el)} wystaje poza okno (${Math.round(rect.right)} > ${width})`);
      // Ucięty tekst: treść szersza niż element, a element jej nie przewija i nie skraca wielokropkiem.
      const style = getComputedStyle(el);
      const cut =
        el.scrollWidth > el.clientWidth + 2 &&
        style.overflowX !== 'visible' &&
        style.overflowX !== 'auto' &&
        style.overflowX !== 'scroll' &&
        style.textOverflow !== 'ellipsis' &&
        el.tagName !== 'INPUT';
      if (cut) defects.push(`${describe(el)} ma ucięty tekst`);
    }
    return [...new Set(defects)];
  });
}

export function watchConsole(page: Page): string[] {
  const errors: string[] = [];
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(`${message.text()} ${message.location().url}`);
  });
  page.on('pageerror', (error) => errors.push(String(error)));
  return errors;
}
