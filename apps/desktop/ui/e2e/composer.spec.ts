// Composer rozmowy: nieudane wysłanie nie kasuje szkicu, udane nie przywraca go
// (uwaga przeglądu PR #1: Q-4).
import { expect, test } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

const TEXT = 'Policz budżet na listopad';

test('nieudane wysłanie: treść wraca do pola i jest komunikat błędu', async ({ page }) => {
  await openApp(page, { scenario: 'send-error' });
  await waitForChat(page);
  const field = page.locator('#alfa-composer');
  await field.fill(TEXT);
  await field.press('Enter');
  await expect(field).toHaveValue(TEXT);
  await expect(page.getByText(/Nie udało się zapisać wiadomości/)).toBeVisible();
});

test('udane wysłanie: pole zostaje puste (bez zdublowania treści)', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  const field = page.locator('#alfa-composer');
  await field.fill(TEXT);
  await field.press('Enter');
  await expect(page.locator('main').getByText(TEXT, { exact: true }).first()).toBeVisible();
  await page.waitForTimeout(300);
  await expect(field).toHaveValue('');
});
