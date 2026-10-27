// Nachodzenie elementów na siebie i tekst ucięty w pionie (zgłoszenie właściciela 2026-10-27:
// „czy nic nie wychodzi, nie ma nachodzenia na siebie, teksty są równe”). Uzupełnia
// `layoutDefects` z `layout.ts`, który sprawdza wystawanie poza okno i ucięcie w poziomie.
import type { Page } from '@playwright/test';

/** Pary widocznych elementów (przyciski, pola, tekst), które zachodzą na siebie na ekranie. */
export async function overlapDefects(page: Page, scope = 'body'): Promise<string[]> {
  return page.evaluate((scopeSelector) => {
    const root = document.querySelector(scopeSelector);
    if (!root) return [`brak obszaru ${scopeSelector}`];
    const defects: string[] = [];
    const describe = (el: Element): string => {
      const text = (el.textContent ?? '').trim().replace(/\s+/g, ' ').slice(0, 30);
      const label = el.getAttribute('aria-label') ?? '';
      const cls = typeof el.className === 'string' ? el.className.split(' ')[0] : '';
      return `<${el.tagName.toLowerCase()}${cls ? `.${cls}` : ''}> „${text || label}”`;
    };
    const ownText = (el: Element): boolean =>
      [...el.childNodes].some((n) => n.nodeType === Node.TEXT_NODE && (n.textContent ?? '').trim());
    // Warstwa: element w menu, liście, oknie dialogowym albo podpowiedzi leży NAD resztą z założenia.
    const LAYER = '[role="dialog"],[role="menu"],[role="listbox"],[role="tooltip"],[popover]';
    /** Prostokąt elementu przycięty przez przodków, którzy przycinają zawartość. */
    const visibleRect = (el: Element): DOMRect | null => {
      // Treść zwiniętego <details> ma wymiary, choć nie jest rysowana — `checkVisibility` ją pomija.
      if (!el.checkVisibility({ opacityProperty: true, visibilityProperty: true })) return null;
      const r = el.getBoundingClientRect();
      let [left, top, right, bottom] = [r.left, r.top, r.right, r.bottom];
      for (let node = el.parentElement; node; node = node.parentElement) {
        const s = getComputedStyle(node);
        if (Number(s.opacity) === 0 || s.visibility === 'hidden') return null;
        if (s.overflowX !== 'visible' || s.overflowY !== 'visible') {
          const c = node.getBoundingClientRect();
          left = Math.max(left, c.left);
          top = Math.max(top, c.top);
          right = Math.min(right, c.right);
          bottom = Math.min(bottom, c.bottom);
        }
      }
      left = Math.max(left, 0);
      top = Math.max(top, 0);
      right = Math.min(right, window.innerWidth);
      bottom = Math.min(bottom, window.innerHeight);
      // Tekst tylko dla czytników ekranu (1 × 1 px) nie jest częścią układu.
      if (right - left <= 2 || bottom - top <= 2) return null;
      return new DOMRect(left, top, right - left, bottom - top);
    };
    const selector =
      'button, a, input, textarea, [role="combobox"], [role="tab"], [role="switch"], [role="radio"], [role="checkbox"], [role="menuitem"], [role="option"], h1, h2, h3, h4, label, p, li, span, small, strong, kbd, code, td, th';
    const items: { el: Element; r: DOMRect; layer: Element | null }[] = [];
    for (const el of root.querySelectorAll(selector)) {
      const control = el.matches(
        'button, a, input, textarea, [role="combobox"], [role="tab"], [role="switch"], [role="radio"], [role="checkbox"], [role="menuitem"], [role="option"]',
      );
      if (!control && !ownText(el)) continue;
      const r = visibleRect(el);
      if (r) items.push({ el, r, layer: el.closest(LAYER) });
    }
    for (let i = 0; i < items.length; i += 1) {
      const a = items[i]!;
      for (let j = i + 1; j < items.length; j += 1) {
        const b = items[j]!;
        if (a.layer !== b.layer) continue;
        if (a.el.contains(b.el) || b.el.contains(a.el)) continue;
        const w = Math.min(a.r.right, b.r.right) - Math.max(a.r.left, b.r.left);
        const h = Math.min(a.r.bottom, b.r.bottom) - Math.max(a.r.top, b.r.top);
        if (w <= 2 || h <= 2) continue;
        // Element przykryty w całości przez inny (np. pod nieprzezroczystym panelem) nie jest
        // widoczny — liczy się tylko nakładanie, które widać: oba trafiane w części wspólnej.
        const x = Math.max(a.r.left, b.r.left) + w / 2;
        const y = Math.max(a.r.top, b.r.top) + h / 2;
        const hit = document.elementFromPoint(x, y);
        // Trafiony wspólny przodek (tło, odstęp) znaczy, że w tym miejscu nie widać żadnego z nich.
        if (!hit || !(a.el.contains(hit) || b.el.contains(hit))) continue;
        defects.push(
          `${describe(a.el)} nachodzi na ${describe(b.el)} (${Math.round(w)}×${Math.round(h)} px)`,
        );
      }
    }
    // Tekst ucięty w pionie: treść wyższa niż element, który jej nie przewija ani nie skraca.
    for (const { el } of items) {
      if (!ownText(el) && !el.matches('button, label, [role="tab"], [role="menuitem"]')) continue;
      const s = getComputedStyle(el);
      const clamps = s.webkitLineClamp !== 'none' && s.webkitLineClamp !== '';
      if (
        el.scrollHeight > el.clientHeight + 2 &&
        (s.overflowY === 'hidden' || s.overflowY === 'clip') &&
        !clamps &&
        el.tagName !== 'INPUT' &&
        el.tagName !== 'TEXTAREA'
      )
        defects.push(
          `${describe(el)} ma tekst ucięty w pionie (${el.scrollHeight} > ${el.clientHeight})`,
        );
    }
    return [...new Set(defects)];
  }, scope);
}
