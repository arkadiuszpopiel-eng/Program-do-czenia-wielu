import { expect, test } from '@playwright/test';
import { expectReadable } from './contrast';
import { waitForChat } from './helpers';

// Plan v6, C2.17: import kluczy API ze zmiennych środowiskowych — przycisk i raport bez wartości.
test('konta: import ze zmiennych środowiskowych pokazuje raport po nazwach zmiennych', async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto('/');
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Konta i klucze API', exact: true })
    .click();
  await page.getByRole('button', { name: 'Importuj ze zmiennych środowiskowych' }).click();
  const report = page.locator('section.env');
  await expect(report.getByRole('heading')).toHaveText(
    'Dodano 1 konto ze zmiennych środowiskowych',
  );
  await expect(report.locator('ul.found li')).toHaveCount(2);
  await expect(report.getByText('ANTHROPIC_API_KEY')).toBeVisible();
  await expect(report.getByText('dodano konto')).toBeVisible();
  await expect(report.getByText('ten klucz jest już zapisany')).toBeVisible();
  await expect(report.getByText('Nieustawione 3 zmienne:')).toBeVisible();
  await expect(report.locator('.vars')).toContainText('GEMINI_API_KEY');
  // Raport nie zawiera niczego, co wygląda jak klucz.
  expect(await report.innerText()).not.toMatch(/sk-[A-Za-z0-9_-]{8,}/);
  await expectReadable(page, 'section.env');
  await report.getByRole('button', { name: 'Zamknij' }).click();
  await expect(report).toHaveCount(0);
});
