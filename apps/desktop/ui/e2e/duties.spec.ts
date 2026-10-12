import { expect, test } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

// Gotowe obowiązki codzienne są ukryte od 0.0.44: próba na lokalnym modelu dała raport z wymyślonymi
// danymi. Formularz wyzwalacza działa jak dotąd; wzory wrócą razem z podawaniem faktów agentce (P14).
test('gotowe obowiązki codzienne nie są pokazywane w formularzu wyzwalacza', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Zadania w tle i wyzwalacze', exact: true })
    .click();
  await expect(page.getByRole('heading', { name: 'Nowy wyzwalacz' })).toBeVisible();
  await expect(page.getByRole('group', { name: 'Gotowe obowiązki codzienne' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Poranny przegląd dnia' })).toHaveCount(0);
  // Własny wyzwalacz nadal da się utworzyć.
  await page.getByRole('textbox', { name: 'Nazwa' }).fill('Mój wyzwalacz');
  await page
    .getByRole('textbox', { name: 'Co ma zrobić agentka' })
    .fill('Sprawdź katalog roboczy.');
  await page.getByRole('button', { name: 'Utwórz wyzwalacz' }).click();
  await expect(page.getByText('Mój wyzwalacz').first()).toBeVisible();
});
