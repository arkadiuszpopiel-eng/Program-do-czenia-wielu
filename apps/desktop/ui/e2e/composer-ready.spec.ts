import { expect, test } from '@playwright/test';
import { prepareComposer } from '../e2e-app/composer-ready';
import { openApp, waitForChat } from './helpers';

test('wąskie okno: Głos nie zasłania menu po przygotowaniu formularza', async ({ page }) => {
  await page.setViewportSize({ width: 1028, height: 779 });
  await openApp(page);
  await waitForChat(page);
  await page.keyboard.press('Alt+6');
  const drawer = page.getByRole('dialog', { name: 'Panel boczny', exact: true });
  await expect(drawer).toBeVisible();
  const composer = await prepareComposer(page);
  // Ten sam zwykły klik; pięć sekund nie jest przedłużane po poprawce prewarunku.
  await composer.getByRole('button', { name: /^Tryb pracy:/ }).click({ timeout: 5000 });
  await expect(page.getByRole('menu', { name: 'Tryb pracy' })).toBeVisible();
  await expect(drawer).toHaveCount(0);
});

test('szerokie okno: przygotowanie formularza zachowuje niemodalny Głos', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await openApp(page);
  await waitForChat(page);
  await page.keyboard.press('Alt+6');
  await expect(page.getByRole('tabpanel', { name: 'Głos' })).toBeVisible();
  await expect(page.getByRole('dialog', { name: 'Panel boczny', exact: true })).toHaveCount(0);
  const composer = await prepareComposer(page);
  await composer.getByRole('button', { name: /^Tryb pracy:/ }).click({ timeout: 5000 });
  await expect(page.getByRole('menu', { name: 'Tryb pracy' })).toBeVisible();
  await expect(page.getByRole('tabpanel', { name: 'Głos' })).toBeVisible();
});
