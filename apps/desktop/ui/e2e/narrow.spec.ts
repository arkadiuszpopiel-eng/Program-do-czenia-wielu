import { expect, test, type Locator, type Page } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

// E.1 — wąskie okno (minimum Tauri 400 px, wąskie ekrany od 360 px): rozmowa bez poziomego
// przewijania, pole pisania w oknie, a panele i okna dialogowe mieszczą się w szerokości okna.

/** Widoczne elementy wychodzące poza prawą krawędź (bez przewijanych w poziomie i kodu). */
async function overflowing(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const width = document.documentElement.clientWidth;
    const out: string[] = [];
    for (const el of document.querySelectorAll<HTMLElement>('body *')) {
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) continue;
      if (el.closest('[data-scroll-x], pre, code, .wk-code, table, .alfa-visually-hidden'))
        continue;
      if (r.right > width + 1) {
        const id = el.id ? `#${el.id}` : '';
        const cls = typeof el.className === 'string' ? `.${el.className.split(' ')[0]}` : '';
        out.push(`${el.tagName.toLowerCase()}${id}${cls} (${Math.round(r.right)} > ${width})`);
      }
    }
    return out.slice(0, 5);
  });
}

async function expectInside(page: Page, target: Locator, what: string) {
  await expect(target).toBeVisible();
  // Szuflady wjeżdżają animacją — mierzymy po jej końcu.
  await target.evaluate((el) =>
    Promise.all(el.getAnimations({ subtree: true }).map((a) => a.finished.catch(() => undefined))),
  );
  const box = await target.boundingBox();
  const width = page.viewportSize()?.width ?? 0;
  expect(box, what).not.toBeNull();
  if (!box) return;
  expect(box.x, `${what}: lewa krawędź`).toBeGreaterThanOrEqual(-1);
  expect(box.x + box.width, `${what}: prawa krawędź`).toBeLessThanOrEqual(width + 1);
}

async function palette(page: Page, command: string) {
  await page.keyboard.press('Control+k');
  await expect(page.locator('dialog[open] input')).toBeFocused();
  await page.keyboard.type(command);
  await expect(page.locator('dialog [role=option][data-selected]')).toContainText(command);
  await page.keyboard.press('Enter');
}

for (const width of [400, 360]) {
  test(`rozmowa w oknie ${width} px: bez poziomego przewijania, pole pisania w oknie`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 700 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await openApp(page);
    await waitForChat(page);
    const scroll = await page.evaluate(() => ({
      scroll: document.documentElement.scrollWidth,
      client: document.documentElement.clientWidth,
    }));
    expect(scroll.scroll, 'poziome przewijanie strony').toBeLessThanOrEqual(scroll.client);
    expect(await overflowing(page), `elementy poza oknem ${width} px`).toEqual([]);
    await expectInside(page, page.locator('#alfa-composer'), 'pole pisania');
    await page.locator('#alfa-composer').fill('Krótka wiadomość z wąskiego okna');
    await page.keyboard.press('Enter');
    await expect(page.getByText('Krótka wiadomość z wąskiego okna')).toBeVisible();
    expect(await overflowing(page), `po wysłaniu (${width} px)`).toEqual([]);
  });

  test(`panele i okna dialogowe w oknie ${width} px mieszczą się w szerokości`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 700 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await openApp(page);
    await waitForChat(page);
    await page.keyboard.press('Control+b');
    await expectInside(page, page.getByRole('dialog', { name: 'Sesje' }), 'panel Sesje');
    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog', { name: 'Sesje' })).toHaveCount(0);
    await page.keyboard.press('Alt+2');
    await expectInside(page, page.getByRole('dialog', { name: 'Panel boczny' }), 'panel boczny');
    await page.keyboard.press('Escape');
    await page.keyboard.press('Control+k');
    await expectInside(page, page.locator('dialog[open]'), 'paleta');
    await page.keyboard.press('Escape');
    for (const [command, title] of [
      ['Statystyki rozmowy', 'Statystyki rozmowy'],
      ['Pokaż Tablicę', 'Tablica'],
    ] as const) {
      await palette(page, command);
      const dialog = page.getByRole('dialog', { name: title });
      await expectInside(page, dialog, title);
      expect(await overflowing(page), `${title} (${width} px)`).toEqual([]);
      await page.keyboard.press('Escape');
      await expect(dialog).toHaveCount(0);
    }
  });
}
