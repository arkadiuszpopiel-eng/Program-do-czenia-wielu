import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

test('belka w320px nie ma zbyt małego celu tytułu rozmowy', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 740 });
  await openApp(page);
  await waitForChat(page);
  await expectAccessible(page, 'belka320px');
});

test('kokpit Jarvis jest opcjonalny, pamięta wybór i nie zmienia trybu zgód', async ({ page }) => {
  await openApp(page, { path: '/?style=next&theme=dark' });
  await waitForChat(page);
  const mode = await page.getByRole('button', { name: /Tryb pracy:/ }).getAttribute('aria-label');
  await page
    .getByRole('button', { name: 'Włącz kokpit Jarvis', exact: true })
    .click({ timeout: 5000 });
  await expect(page.getByRole('region', { name: 'Kokpit Jarvis', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: /Tryb pracy:/ })).toHaveAttribute(
    'aria-label',
    mode!,
  );
  await page.reload();
  await waitForChat(page);
  await expect(page.getByRole('region', { name: 'Kokpit Jarvis', exact: true })).toBeVisible();
  await expectAccessible(page, 'kokpit Jarvis');
  await page.getByRole('button', { name: 'Wyłącz kokpit Jarvis', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Kokpit Jarvis', exact: true })).toHaveCount(0);
});

for (const style of ['classic', 'glass', 'next', 'junior']) {
  for (const theme of ['light', 'dark'] as const) {
    for (const width of [390, 1280]) {
      test(`kafelki Jarvisa ${style} ${theme} ${width}px`, async ({ page }) => {
        await page.setViewportSize({ width, height: 900 });
        await openApp(page, { theme, path: `/?style=${style}&theme=${theme}` });
        await waitForChat(page);
        await page.getByRole('button', { name: 'Włącz kokpit Jarvis', exact: true }).click();
        const cockpit = page.getByRole('region', { name: 'Kokpit Jarvis', exact: true });
        await expect(cockpit).toBeVisible();
        const tiles = cockpit.locator('[data-cockpit-tile]');
        await expect(tiles).toHaveCount(6);
        for (const tile of await tiles.all()) {
          const box = await tile.boundingBox();
          expect(box?.height).toBeGreaterThanOrEqual(44);
          expect(box?.width).toBeGreaterThanOrEqual(44);
          expect(box && box.x >= 0 && box.x + box.width <= width + 1).toBeTruthy();
        }
        await expect(page.locator('#alfa-composer')).toBeInViewport();
        await expectAccessible(page, `Jarvis ${style} ${theme} ${width}`);
      });
    }
  }
}

test('kafelki otwierają panel agentek, zadań i ustawienia głosu', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await page.getByRole('button', { name: 'Włącz kokpit Jarvis', exact: true }).click();
  await page.locator('[data-cockpit-tile="agents"]').click();
  await expect(page.getByRole('tab', { name: 'Agentki', exact: true })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await page.locator('[data-cockpit-tile="tasks"]').click();
  await expect(page.getByRole('tab', { name: 'Zadania', exact: true })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await page.locator('[data-cockpit-tile="voice"]').click();
  await expect(page.locator('[data-page="voice"]')).toHaveAttribute('aria-current', 'page');
  for (const [tile, setting] of [
    ['models', 'models'],
    ['access', 'permissions'],
  ]) {
    await page.getByRole('button', { name: 'Wróć do rozmowy', exact: true }).click();
    await page.locator(`[data-cockpit-tile="${tile}"]`).click();
    await expect(page.locator(`[data-page="${setting}"]`)).toHaveAttribute('aria-current', 'page');
  }
});

test.describe('telefon dotykowy', () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
  test('belka telefonu ma dostępne cele dotykowe', async ({ page }) => {
    await openApp(page);
    await waitForChat(page);
    const sessions = page.getByRole('button', { name: 'Pokaż panel Sesje', exact: true });
    const rect = await sessions.boundingBox();
    expect(rect?.width).toBeGreaterThanOrEqual(44);
    expect(rect?.height).toBeGreaterThanOrEqual(44);
    await expect(page.getByRole('group', { name: 'Okno', exact: true })).toBeHidden();
  });
});

for (const viewport of [
  { width: 320, height: 568 },
  { width: 844, height: 390 },
]) {
  test.describe(`telefon ${viewport.width}x${viewport.height}`, () => {
    test.use({ viewport, hasTouch: true, isMobile: true });
    test('kafelki i composer mieszczą się także w niskim oknie', async ({ page }) => {
      await openApp(page);
      await waitForChat(page);
      await page.getByRole('button', { name: 'Włącz kokpit Jarvis', exact: true }).click();
      await expect(page.getByRole('region', { name: 'Kokpit Jarvis', exact: true })).toBeVisible();
      for (const tile of await page.locator('[data-cockpit-tile]').all()) {
        const box = await tile.boundingBox();
        expect(box && box.x >= 0 && box.x + box.width <= viewport.width + 1).toBeTruthy();
      }
      await expect(page.locator('#alfa-composer')).toBeInViewport();
      await expectAccessible(page, `telefon ${viewport.width}`);
    });
  });
}
