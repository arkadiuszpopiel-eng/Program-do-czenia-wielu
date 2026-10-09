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

// Panel prawy ma 7 kart; w wąskim panelu dwie ostatnie („Ekran”, „Głos”) wypadały poza krawędź
// bez paska przewijania — nie dało się do nich dojść myszą.
for (const width of [1280, 1600]) {
  test(`panel prawy w oknie ${width} px: wszystkie karty widoczne i klikalne`, async ({ page }) => {
    await page.setViewportSize({ width, height: 800 });
    await openApp(page);
    await waitForChat(page);
    await page.keyboard.press('Alt+1');
    const tablist = page.getByRole('tablist', { name: 'Panel boczny' });
    await expect(tablist).toBeVisible();
    const tabs = tablist.getByRole('tab');
    await expect(tabs).toHaveCount(7);
    const box = await tablist.boundingBox();
    expect(box).not.toBeNull();
    for (const tab of await tabs.all()) {
      const r = await tab.boundingBox();
      const name = await tab.innerText();
      expect(r, name).not.toBeNull();
      if (!r || !box) continue;
      expect(r.x, `${name}: lewa krawędź`).toBeGreaterThanOrEqual(box.x - 1);
      expect(r.x + r.width, `${name}: prawa krawędź`).toBeLessThanOrEqual(box.x + box.width + 1);
    }
    await tabs.filter({ hasText: 'Głos' }).click();
    await expect(tabs.filter({ hasText: 'Głos' })).toHaveAttribute('aria-selected', 'true');
  });
}

test('panel prawy na szerokim oknie: wszystkie karty w jednym wierszu', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await openApp(page);
  await page.keyboard.press('Alt+1');
  const tabs = page.getByRole('tablist').first().getByRole('tab');
  await expect(tabs).toHaveCount(7);
  const tops = new Set<number>();
  for (const tab of await tabs.all()) {
    const r = await tab.boundingBox();
    if (r) tops.add(Math.round(r.y));
  }
  expect([...tops], 'karty na jednej wysokości').toHaveLength(1);
});

// Skala interfejsu (CSS `zoom`) mnoży jednostki okna: `min-height: 100vh` robiło stronę wyższą niż
// okno i pole pisania znikało pod dolną krawędzią przy każdej skali powyżej 100 %.
for (const [width, height] of [
  [1920, 1080],
  [2560, 1440],
  [3840, 2160],
] as const) {
  test(`skala dopasowana do okna ${width}×${height}: strona ma wysokość okna, pole pisania widoczne`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height });
    await openApp(page);
    await waitForChat(page);
    const m = await page.evaluate(() => {
      const composer = document.querySelector('#alfa-composer');
      return {
        zoom: Number(document.documentElement.style.zoom),
        scroll: document.documentElement.scrollHeight,
        body: document.body.getBoundingClientRect().height,
        bottom: composer ? composer.getBoundingClientRect().bottom : -1,
      };
    });
    expect(m.zoom, 'skala').toBeGreaterThan(1);
    expect(Math.round(m.body), 'wysokość strony').toBeLessThanOrEqual(height);
    expect(m.scroll, 'brak przewijania całej strony').toBeLessThanOrEqual(height);
    expect(m.bottom, 'dół pola pisania').toBeGreaterThan(0);
    expect(m.bottom, 'dół pola pisania').toBeLessThanOrEqual(height);
  });
}

// Przy starcie obszar rozmowy zmienia wysokość (dochodzi pole pisania, przelicza się skala).
// Przewinięcie wywołane samym układem strony odklejało listę od dołu — rozmowa otwierała się
// kilkaset pikseli nad ostatnią wiadomością (widoczne przy ograniczonym ruchu i skali > 100 %).
for (const [width, height] of [
  [1920, 1080],
  [2560, 1440],
] as const) {
  test(`rozmowa otwiera się na ostatniej wiadomości (${width}×${height}, ograniczony ruch)`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await openApp(page);
    await waitForChat(page);
    await page.waitForTimeout(800);
    const { gap, scrollable } = await page.evaluate(() => {
      const feed = document.querySelector('[role=feed]') as HTMLElement;
      return {
        gap: feed.scrollHeight - feed.scrollTop - feed.clientHeight,
        scrollable: feed.scrollHeight - feed.clientHeight,
      };
    });
    // Bez przewijania odległość od dołu zawsze wynosi 0 i test niczego by nie sprawdzał.
    expect(scrollable, 'rozmowa dłuższa niż okno').toBeGreaterThan(100);
    expect(gap, 'odległość od dołu').toBeLessThanOrEqual(24);
    await expect(page.getByRole('button', { name: /Przewiń do najnowszych/ })).toHaveCount(0);
  });
}

test('kółko myszy odkleja rozmowę od dołu, przycisk wraca na dół', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 600 });
  await openApp(page);
  await waitForChat(page);
  const feed = page.getByRole('feed');
  await feed.hover();
  await page.mouse.wheel(0, -600);
  const jump = page.getByRole('button', { name: /Przewiń do najnowszych/ });
  await expect(jump).toBeVisible();
  // Zmiana rozmiaru okna nie przykleja odklejonej listy z powrotem.
  await page.setViewportSize({ width: 1280, height: 640 });
  await expect(jump).toBeVisible();
  await jump.click();
  await expect(jump).toHaveCount(0);
});
