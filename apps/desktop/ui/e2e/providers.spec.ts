import { expect, test, type Page } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

// Kreator „Dodaj dostawcę" — regresja z testu na laptopie (2026-10-07): odmowa rdzenia (brak
// endpointu) kasowała klucz bez żadnego komunikatu.

async function openWizard(page: Page) {
  await openApp(page, { scenario: 'no-keys' });
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Konta i klucze API' })
    .click();
  await page.getByRole('button', { name: 'Dodaj dostawcę' }).click();
  await expect(page.getByRole('heading', { name: 'Dodaj dostawcę' })).toBeVisible();
}

test('dostawca ze znanym endpointem: bez pola adresu, klucz zapisany, test połączenia', async ({
  page,
}) => {
  await openWizard(page);
  await page.getByRole('button', { name: /Anthropic \(Claude\)/ }).click();
  await expect(page.getByLabel('Adres endpointu (base URL)')).toHaveCount(0);
  await page.getByLabel('Klucz API').fill('sk-ant-test-0000');
  await page.getByRole('button', { name: 'Sprawdź klucz' }).click();
  await expect(page.getByText(/^Połączono/)).toBeVisible();
});

test('dostawca z adresem w katalogu (DeepSeek) nie pyta o endpoint', async ({ page }) => {
  await openWizard(page);
  await page.getByRole('button', { name: /DeepSeek/ }).click();
  await expect(page.getByLabel('Adres endpointu (base URL)')).toHaveCount(0);
  await page.getByLabel('Klucz API').fill('sk-test-0000-0000-0000');
  await page.getByRole('button', { name: 'Sprawdź klucz' }).click();
  await expect(page.getByText(/^Połączono/)).toBeVisible();
});

test('własny endpoint: bez adresu nie da się wysłać klucza; zły adres daje komunikat, klucz zostaje', async ({
  page,
}) => {
  await openWizard(page);
  await page.getByRole('button', { name: /Kimi/ }).click();
  const key = page.getByLabel('Klucz API');
  const submit = page.getByRole('button', { name: 'Sprawdź klucz' });
  await key.fill('sk-test-0000-0000-0000');
  await expect(submit).toBeDisabled();
  // Adres bez szyfrowania przechodzi przez pole, ale rdzeń go odrzuca — komunikat, nie cisza.
  await page.getByLabel('Adres endpointu (base URL)').fill('http://api.example.com');
  await submit.click();
  await expect(page.getByRole('alert')).toContainText('Nie zapisano klucza');
  await expect(page.getByRole('alert')).toContainText('https://');
  await expect(key).toHaveValue('sk-test-0000-0000-0000');
  // Dane logowania przed `@` udają host lokalny — rdzeń tego nie przepuszcza.
  await page.getByLabel('Adres endpointu (base URL)').fill('http://localhost:80@evil.example/v1');
  await submit.click();
  await expect(page.getByRole('alert')).toContainText('https://');
  await page.getByLabel('Adres endpointu (base URL)').fill('https://api.example.com/v1');
  await submit.click();
  await expect(page.getByText(/^Połączono/)).toBeVisible();
});
