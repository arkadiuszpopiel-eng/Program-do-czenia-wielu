import { expect, test, type Page } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

async function enlarge(page: Page, steps: number) {
  const zoom = () =>
    page.evaluate(() => Number.parseFloat(getComputedStyle(document.documentElement).zoom));
  const before = await zoom();
  for (let i = 0; i < steps; i++) await page.keyboard.press('Control+=');
  await expect.poll(zoom).toBeGreaterThan(before);
}

// Regresja CI149: radio zakresu nad oknem; bez force i bez rozszerzania limitu czasu.
for (const style of ['classic', 'glass', 'next', 'junior']) {
  for (const height of [500, 650, 800]) {
    test(`menu pięciu trybów mieści się w pionie — ${style} ${height}px`, async ({ page }) => {
      await page.setViewportSize({ width: 1280, height });
      await openApp(page, { path: `/?style=${style}` });
      await waitForChat(page);
      for (const larger of [false, true]) {
        if (larger) {
          await enlarge(page, 5);
        }
        await page.getByRole('button', { name: /Tryb pracy:/ }).click();
        const menu = page.getByRole('dialog', { name: 'Tryb pracy', exact: true });
        await expect(menu).toBeVisible();
        const choices = menu.getByRole('menuitemradio');
        await expect(choices).toHaveCount(5);
        await page.screenshot({
          path: test.info().outputPath(`mode-${larger ? 'larger' : 'normal'}.png`),
        });
        const box = await menu.boundingBox();
        expect(box).not.toBeNull();
        if (!box) return;
        expect(box.y, `górna krawędź, powiększenie=${larger}`).toBeGreaterThanOrEqual(0);
        expect(box.y + box.height, 'dolna krawędź').toBeLessThanOrEqual(height);
        await menu.getByRole('radio', { name: 'Wszystkie rozmowy', exact: true }).check();
        await menu.getByRole('radio', { name: 'Ta rozmowa', exact: true }).check();
        for (const choice of await choices.all()) {
          await choice.scrollIntoViewIfNeeded();
          await expect(choice).toBeInViewport({ ratio: 1 });
        }
        await menu.getByRole('menuitemradio', { name: /Ręcznie/ }).click();
        await expect(menu).toHaveCount(0);
      }
    });
  }
}

for (const style of ['classic', 'glass', 'next', 'junior']) {
  test(`menu pięciu trybów mieści się w360px — ${style}`, async ({ page }) => {
    await page.setViewportSize({ width: 360, height: 800 });
    await openApp(page, { path: `/?style=${style}` });
    await waitForChat(page);
    await page.getByRole('button', { name: /Tryb pracy:/ }).click();
    const menu = page.getByRole('dialog', { name: 'Tryb pracy', exact: true });
    const choices = menu.getByRole('menuitemradio');
    await expect(choices).toHaveCount(5);
    await page.screenshot({ path: test.info().outputPath('mode-narrow.png') });
    const box = await menu.boundingBox();
    expect(box).not.toBeNull();
    if (!box) return;
    expect(box.x, 'lewa krawędź').toBeGreaterThanOrEqual(0);
    expect(box.x + box.width, 'prawa krawędź').toBeLessThanOrEqual(360);
    await menu.getByRole('radio', { name: 'Ta rozmowa', exact: true }).check();
    for (const choice of await choices.all()) {
      await choice.scrollIntoViewIfNeeded();
      await expect(choice).toBeInViewport({ ratio: 1 });
    }
    await menu.getByRole('menuitemradio', { name: /Ręcznie/ }).click();
    await expect(menu).toHaveCount(0);
  });
}

test('otwarte menu dopasowuje się do zmiany wysokości i skali', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await page.getByRole('button', { name: /Tryb pracy:/ }).click();
  const menu = page.getByRole('dialog', { name: 'Tryb pracy', exact: true });
  await expect(menu.getByRole('menuitemradio')).toHaveCount(5);
  await page.setViewportSize({ width: 1280, height: 500 });
  for (const larger of [false, true]) {
    if (larger) {
      await enlarge(page, 3);
    }
    await expect
      .poll(async () => {
        const box = await menu.boundingBox();
        return box !== null && box.y >= 0 && box.y + box.height <= 500;
      })
      .toBe(true);
    await menu.getByRole('radio', { name: 'Wszystkie rozmowy', exact: true }).check();
    await menu.getByRole('radio', { name: 'Ta rozmowa', exact: true }).check();
  }
  await page.keyboard.press('Escape');
  await expect(menu).toHaveCount(0);
});
