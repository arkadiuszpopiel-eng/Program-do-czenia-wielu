import { expect, type Page } from '@playwright/test';

/**
 * Kontrast tekstu po złożeniu warstw. Axe nie ocenia tekstu nad półprzezroczystą powierzchnią albo
 * gradientem (zgłasza „nie da się ocenić”, a nie naruszenie), więc w stylach z takimi tłami test
 * dostępności przechodził także przy nieczytelnym tekście. Tu każde tło z kanałem alfa jest
 * składane z tym, co pod nim, aż do koloru bazowego strony; próg WCAG AA: 4,5 (duży tekst: 3).
 * Pomijane: elementy wyłączone i przygaszone (zwolnione z wymogu) oraz tekst na gradiencie.
 */
export async function expectReadable(page: Page, label: string) {
  const failures = await page.evaluate(() => {
    const probe = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
    const rgba = (color: string): number[] => {
      if (!probe) return [0, 0, 0, 0];
      probe.clearRect(0, 0, 1, 1);
      probe.fillStyle = '#000';
      probe.fillStyle = color;
      probe.fillRect(0, 0, 1, 1);
      const [r = 0, g = 0, b = 0, a = 0] = probe.getImageData(0, 0, 1, 1).data;
      return [r, g, b, a / 255];
    };
    const over = (top: number[], under: number[]): number[] => {
      const a = top[3] ?? 0;
      return [0, 1, 2].map((i) => (top[i] ?? 0) * a + (under[i] ?? 0) * (1 - a)).concat(1);
    };
    const lum = (c: number[]) => {
      const ch = (v: number) => {
        const x = v / 255;
        return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
      };
      return 0.2126 * ch(c[0] ?? 0) + 0.7152 * ch(c[1] ?? 0) + 0.0722 * ch(c[2] ?? 0);
    };
    const root = getComputedStyle(document.documentElement);
    const base = rgba(root.getPropertyValue('--alfa-color-bg') || root.backgroundColor);
    /** Tło pod elementem; `null`, gdy po drodze jest gradient albo obraz (nie do policzenia). */
    const backdrop = (el: Element): number[] | null => {
      const layers: number[][] = [];
      for (let node: Element | null = el; node; node = node.parentElement) {
        const style = getComputedStyle(node);
        if (style.backgroundImage !== 'none' && node !== document.body) return null;
        const color = rgba(style.backgroundColor);
        if ((color[3] ?? 0) > 0) layers.push(color);
        if ((color[3] ?? 0) >= 0.999) break;
      }
      let result = [base[0] ?? 255, base[1] ?? 255, base[2] ?? 255, 1];
      for (const layer of layers.reverse()) result = over(layer, result);
      return result;
    };
    const exempt = (el: Element): boolean => {
      for (let node: Element | null = el; node; node = node.parentElement) {
        if (Number(getComputedStyle(node).opacity) < 1) return true;
        if (node.matches(':disabled, [aria-disabled="true"], [aria-hidden="true"], [inert]'))
          return true;
      }
      return el.closest('.alfa-visually-hidden') !== null;
    };
    const bad: string[] = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    const seen = new Set<Element>();
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const el = node.parentElement;
      const text = node.textContent?.trim() ?? '';
      if (!el || seen.has(el) || !text) continue;
      seen.add(el);
      const rect = el.getBoundingClientRect();
      const style = getComputedStyle(el);
      if (rect.width === 0 || rect.height === 0 || style.visibility === 'hidden') continue;
      if (rect.bottom < 0 || rect.top > window.innerHeight || exempt(el)) continue;
      const bg = backdrop(el);
      if (!bg) continue;
      const fg = over(rgba(style.color), bg);
      const [hi, lo] = lum(fg) > lum(bg) ? [lum(fg), lum(bg)] : [lum(bg), lum(fg)];
      const ratio = (hi + 0.05) / (lo + 0.05);
      const size = Number.parseFloat(style.fontSize);
      const large = size >= 24 || (size >= 18.66 && Number(style.fontWeight) >= 700);
      if (ratio < (large ? 3 : 4.5)) {
        const name = el.className.toString().split(' ')[0] || el.tagName.toLowerCase();
        bad.push(`${ratio.toFixed(2)} „${text.slice(0, 28)}” (${name})`);
      }
    }
    return bad;
  });
  expect(failures, `kontrast: ${label}`).toEqual([]);
}
