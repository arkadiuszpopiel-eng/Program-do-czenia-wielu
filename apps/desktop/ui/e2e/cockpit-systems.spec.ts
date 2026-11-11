import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

test('mapa i systemy otwierają się z kokpitu bez zmiany trybu zgód', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  const mode = await page.getByRole('button', { name: /Tryb pracy:/ }).getAttribute('aria-label');
  await page.getByRole('button', { name: 'Włącz kokpit Jarvis', exact: true }).click();
  const launch = page.getByRole('button', { name: 'Mapa i systemy', exact: true });
  await launch.click();
  const dialog = page.getByRole('dialog', { name: 'Mapa i systemy', exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText('Mapa offline · Natural Earth', { exact: true })).toBeVisible();
  await expect(dialog.getByText('Bez śledzenia lokalizacji', { exact: true })).toBeVisible();
  await expect(dialog.locator('[data-system="remote"]')).toContainText('Brak połączenia');
  await expectAccessible(page, 'mapa i systemy');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await expect(launch).toBeFocused();
  await expect(page.getByRole('button', { name: /Tryb pracy:/ })).toHaveAttribute(
    'aria-label',
    mode!,
  );
});

for (const viewport of [
  { width: 320, height: 568 },
  { width: 390, height: 844 },
  { width: 844, height: 390 },
  { width: 1280, height: 900 },
]) {
  for (const theme of ['light', 'dark'] as const) {
    test(`atlas ${viewport.width}x${viewport.height} ${theme}: offline, dotyk i klawiatura`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      const external: string[] = [];
      page.on('request', (r) => {
        if (!r.url().startsWith('http://127.0.0.1:4173/')) external.push(r.url());
      });
      await openApp(page, { theme, path: `/?style=next&theme=${theme}` });
      await waitForChat(page);
      await page.getByRole('button', { name: 'Włącz kokpit Jarvis', exact: true }).click();
      await expect(page.locator('#alfa-composer')).toBeInViewport();
      const launch = page.getByRole('button', { name: 'Mapa i systemy', exact: true });
      const target = await launch.boundingBox();
      expect(target?.height).toBeGreaterThanOrEqual(44);
      expect(target?.width).toBeGreaterThanOrEqual(44);
      expect(target && target.x >= 0 && target.x + target.width <= viewport.width).toBeTruthy();
      await launch.focus();
      await page.keyboard.press('Enter');
      const dialog = page.getByRole('dialog', { name: 'Mapa i systemy', exact: true });
      await expect(dialog).toBeVisible();
      if (viewport.width <= 640) await expect(dialog.locator('.systems-head button')).toBeHidden();
      for (const button of await dialog.getByRole('button').all()) {
        const box = await button.boundingBox();
        if (box) {
          expect(box.height, 'cel dotykowy panelu').toBeGreaterThanOrEqual(44);
          expect(box.width, 'cel dotykowy panelu').toBeGreaterThanOrEqual(44);
        }
      }
      const bounds = await dialog.boundingBox();
      expect(
        bounds &&
          bounds.x >= 0 &&
          bounds.y >= 0 &&
          bounds.x + bounds.width <= viewport.width + 1 &&
          bounds.y + bounds.height <= viewport.height + 1,
      ).toBeTruthy();
      expect(await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBeTruthy();
      await page.screenshot({ path: test.info().outputPath('atlas.png') });
      await expectAccessible(page, `atlas ${viewport.width} ${theme}`);
      await dialog
        .locator('.systems-actions')
        .getByRole('button', { name: 'Wróć do rozmowy', exact: true })
        .click();
      await expect(launch).toBeFocused();
      expect(external).toEqual([]);
    });
  }
}

test('atlas pokazuje utratę Brokera i otwiera prawdziwą stronę diagnostyki', async ({ page }) => {
  await openApp(page, { scenario: 'broker-lost' });
  await waitForChat(page);
  await page.getByRole('button', { name: 'Włącz kokpit Jarvis', exact: true }).click();
  await page.getByRole('button', { name: 'Mapa i systemy', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Mapa i systemy', exact: true });
  await expect(dialog.locator('[data-system="broker"]')).toContainText(
    'Brak połączenia · zgody odrzucane',
  );
  await expect(dialog.locator('[data-system="broker"]')).toHaveAttribute('data-warning', 'true');
  await dialog.getByRole('button', { name: 'Otwórz diagnostykę', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator('[data-page="improve"]')).toHaveAttribute('aria-current', 'page');
});

test('atlas ma angielskie etykiety i działający skrót modeli', async ({ page }) => {
  await openApp(page, { path: '/?locale=en' });
  await waitForChat(page);
  await page.getByRole('button', { name: 'Enable Jarvis cockpit', exact: true }).click();
  await page.getByRole('button', { name: 'Map and systems', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Map and systems', exact: true });
  await expect(dialog.locator('[data-system="remote"]')).toContainText('Not connected');
  await expect(dialog.locator('[data-system="health"]')).toContainText('Status unknown');
  await expectAccessible(page, 'atlas EN');
  await dialog.getByRole('button', { name: 'Model settings', exact: true }).click();
  await expect(page.locator('[data-page="models"]')).toHaveAttribute('aria-current', 'page');
});
