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
    .getByRole('button', { name: 'Modele i dostawcy' })
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
  await page.getByRole('button', { name: 'Dalej' }).click();
  await expect(page.getByText(/^Połączono/)).toBeVisible();
});

test('brak endpointu: komunikat zamiast cichej porażki, klucz zostaje, poprawka przechodzi', async ({
  page,
}) => {
  await openWizard(page);
  await page.getByRole('button', { name: /DeepSeek/ }).click();
  const key = page.getByLabel('Klucz API');
  await key.fill('sk-test-0000');
  await page.getByRole('button', { name: 'Dalej' }).click();
  await expect(page.getByRole('alert')).toContainText('Nie zapisano klucza');
  await expect(page.getByRole('alert')).toContainText('endpointu');
  await expect(key).toHaveValue('sk-test-0000');
  await page.getByLabel('Adres endpointu (base URL)').fill('https://api.deepseek.com');
  await page.getByRole('button', { name: 'Dalej' }).click();
  await expect(page.getByText(/^Połączono/)).toBeVisible();
});
