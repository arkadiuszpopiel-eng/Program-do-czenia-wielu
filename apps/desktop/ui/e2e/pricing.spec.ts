import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

// Cennik modeli (Ustawienia → Koszty i limity, zgłoszenie właściciela 2026-10-20: koszty 0,00 zł):
// ceny z datą i źródłem, aktualizacja z wklejonego tekstu — podgląd zmian, zapis dopiero po
// „Zastosuj”; błędny tekst niczego nie zmienia.

const UPDATE = `checked = "2026-11-01"

[[price]]
provider = "openai"
model = "gpt-6.1-sol"
input = 1.5
output = 9.0
source = "https://developers.openai.com/api/docs/pricing"

[[price]]
provider = "openai"
model = "gpt-nowy"
input = 0.5
output = 1.0
source = "https://developers.openai.com/api/docs/pricing"
`;

async function openCosts(page: Page) {
  await page.keyboard.press('Control+,');
  const name = 'Koszty i limity';
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name, exact: true })
    .click();
  await expect(page.getByRole('heading', { name: 'Cennik modeli', level: 3 })).toBeVisible();
}

for (const theme of ['light', 'dark'] as const) {
  test(`cennik modeli: ceny ze źródłem, podgląd i zapis aktualizacji — ${theme}`, async ({
    page,
  }) => {
    await openApp(page, { theme });
    await waitForChat(page);
    await openCosts(page);
    await expect(page.getByText(/Ceny sprawdzone 2026-10-20 · 5 modeli/)).toBeVisible();
    const table = page.getByRole('region', { name: 'Tabela cen' });
    await expect(table.getByText('claude-haiku-5-5')).toBeVisible();
    await expect(table.getByText('developers.openai.com').first()).toBeVisible();
    await expectAccessible(page, `cennik ${theme}`);

    await page.getByRole('button', { name: 'Aktualizuj ceny' }).click();
    const field = page.getByRole('textbox', { name: /Wklej aktualizację cennika/ });
    // Błędny tekst: komunikat, nic nie zapisano.
    await field.fill('[[price]]\nprovider = "openai"');
    await page.getByRole('button', { name: 'Sprawdź zmiany' }).click();
    await expect(page.getByRole('alert')).toContainText('cennik');
    await expect(page.getByRole('button', { name: 'Zastosuj' })).toBeDisabled();

    await field.fill(UPDATE);
    await page.getByRole('button', { name: 'Sprawdź zmiany' }).click();
    const status = page.getByRole('status').filter({ hasText: 'Zmiany (2)' });
    await expect(status).toContainText('openai / gpt-6.1-sol');
    await expect(status).toContainText('nowy model:');
    await expectAccessible(page, `cennik podgląd ${theme}`);
    // Podgląd niczego nie zmienia.
    await expect(table.getByText('gpt-nowy')).toHaveCount(0);

    await page.getByRole('button', { name: 'Zastosuj' }).click();
    await expect(page.getByText('Zapisano cennik — zmienione ceny: 2.')).toBeVisible();
    await expect(table.getByText('gpt-nowy')).toBeVisible();
    await expect(page.getByText(/Ceny sprawdzone 2026-11-01 · 6 modeli/)).toBeVisible();
    await expect(table.getByText('zaktualizowana', { exact: false }).first()).toBeVisible();
  });
}

test('obowiązek „Kontrola cenników” jest wśród gotowych wzorów', async ({ page }) => {
  await openApp(page, { theme: 'light' });
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Zadania w tle i wyzwalacze', exact: true })
    .click();
  await expect(page.getByText('Kontrola cenników').first()).toBeVisible();
});
