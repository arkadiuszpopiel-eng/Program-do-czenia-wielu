import { expect, test, type Page } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

// Układ w wąskim oknie (PLAN §14.2: tryb kompaktowy < 720 px, minimum okna 400 × 500): żadna strona
// ustawień nie może wychodzić w poziomie poza okno (ucięte przyciski, poziomy pasek przewijania).

/** Elementy, których prawa krawędź wychodzi poza okno (z pominięciem ukrytych i przewijanych). */
async function overflowing(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const width = document.documentElement.clientWidth;
    const out: string[] = [];
    for (const el of document.querySelectorAll<HTMLElement>('main *')) {
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) continue;
      if (el.closest('[data-scroll-x], pre, code, .wk-code, table')) continue;
      if (r.right > width + 1) {
        const id = el.id ? `#${el.id}` : '';
        const cls = typeof el.className === 'string' ? `.${el.className.split(' ')[0]}` : '';
        out.push(`${el.tagName.toLowerCase()}${id}${cls} (${Math.round(r.right)} > ${width})`);
      }
    }
    return out.slice(0, 5);
  });
}

test.describe.configure({ timeout: 180_000 });

for (const width of [720, 480]) {
  test(`wszystkie strony ustawień mieszczą się w oknie ${width} px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 800 });
    await openApp(page);
    await waitForChat(page);
    await page.keyboard.press('Control+,');
    const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
    await expect(nav.getByRole('button').first()).toBeVisible();
    await expect(nav.getByRole('button').first()).toBeVisible();
    const names = await nav.getByRole('button').allInnerTexts();
    expect(names.length).toBeGreaterThan(20);
    const report: string[] = [];
    for (const [i, raw] of names.entries()) {
      const name = raw.split('\n')[0]?.trim() ?? raw;
      await nav.getByRole('button').nth(i).click();
      await expect(page.getByRole('heading', { level: 2, name })).toBeVisible();
      await page.waitForTimeout(250);
      const bad = await overflowing(page);
      if (bad.length) report.push(`${name}: ${bad.join(', ')}`);
    }
    expect(report, `strony wychodzące poza okno ${width} px`).toEqual([]);
  });
}

for (const width of [1280, 720, 480, 400]) {
  test(`pasek tytułu w oknie ${width} px: nic nie łamie się ani nie wychodzi poza pasek`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 700 });
    await openApp(page);
    await waitForChat(page);
    const outside = await page.evaluate(() => {
      const bar = document.querySelector('header, [role="banner"]');
      if (!bar) return ['brak paska tytułu'];
      const box = bar.getBoundingClientRect();
      const out: string[] = [];
      for (const el of bar.querySelectorAll<HTMLElement>('*')) {
        const r = el.getBoundingClientRect();
        if (r.width === 0 || r.height === 0) continue;
        if (r.top < box.top - 1 || r.bottom > box.bottom + 1 || r.right > window.innerWidth + 1) {
          out.push(
            `${el.tagName.toLowerCase()}.${String(el.className).split(' ')[0]}: ${el.textContent?.trim().slice(0, 30)}`,
          );
        }
      }
      // Tekst złamany na kilka linii (np. kapsuła „Delta steruje ekranem” w wąskim oknie).
      const walker = document.createTreeWalker(bar, NodeFilter.SHOW_TEXT);
      for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        const text = node.textContent?.trim() ?? '';
        if (!text) continue;
        const range = document.createRange();
        range.selectNodeContents(node);
        const lines = new Set([...range.getClientRects()].map((r) => Math.round(r.top)));
        if (lines.size > 1) out.push(`tekst w ${lines.size} liniach: ${text.slice(0, 30)}`);
      }
      return out.slice(0, 5);
    });
    expect(outside, `pasek tytułu ${width} px`).toEqual([]);
  });
}
