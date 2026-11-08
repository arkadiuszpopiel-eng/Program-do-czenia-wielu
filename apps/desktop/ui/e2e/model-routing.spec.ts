import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, waitForChat } from './helpers';

// Ustawienia → Dobór modelu (zgłoszenie właściciela 2026-10-14, audyt A-02): ustawienia doboru
// według trudności, szczeble modeli i podgląd na przykładzie (atrapa — te same reguły co rdzeń).

async function openRouting(page: Page) {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto('/');
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Dobór modelu' })
    .click();
  await expect(page.getByRole('heading', { name: 'Dobór modelu', level: 2 })).toBeVisible();
}

test('ustawienia doboru: przełączniki, strategia, sprawdzanie, szczebel ręczny', async ({
  page,
}) => {
  await openRouting(page);
  const enabled = page.getByRole('switch', { name: 'Dobieraj model do trudności zadania' });
  await expect(enabled).toBeChecked();
  await expect(
    page.getByRole('switch', { name: 'Zmieniaj też kolejność kolejek ustawionych ręcznie' }),
  ).not.toBeChecked();
  await expect(page.getByRole('radio', { name: /Zrównoważenie/ })).toBeChecked();
  await expectAccessible(page, 'dobór modelu');

  await page.getByRole('radio', { name: /Jakość/ }).click();
  await expect(page.getByRole('radio', { name: /Jakość/ })).toBeChecked();
  await enabled.click();
  await expect(enabled).not.toBeChecked();
  await enabled.click();
  await expect(enabled).toBeChecked();

  // Szczebel ręczny pierwszego modelu na liście.
  const first = page.locator('ul.models > li').first();
  await expect(first).toBeVisible();
  const select = first.getByRole('combobox');
  await select.click();
  await page.getByRole('option', { name: 'mocny', exact: true }).click();
  await expect(select).toContainText('mocny');
  // Po ponownym otwarciu strony ustawienia zostały zapisane.
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Kolejki modeli API' })
    .click();
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Dobór modelu' })
    .click();
  await expect(page.getByRole('radio', { name: /Jakość/ })).toBeChecked();
  await expect(page.locator('ul.models > li').first().getByRole('combobox')).toContainText('mocny');
});

test('podgląd: łatwe pytanie, kod nie schodzi poniżej średniego, kolejność prób', async ({
  page,
}) => {
  await openRouting(page);
  const text = page.getByLabel('Przykładowa prośba');
  const check = page.getByRole('button', { name: 'Sprawdź', exact: true });
  await expect(check).toBeDisabled();
  await text.fill('Która jest godzina?');
  await check.click();
  const result = page.locator('.result');
  await expect(result).toContainText('Zadanie łatwe: krótka, prosta prośba');
  await expect(result.getByRole('listitem').first()).toContainText('odpowie pierwszy');

  await page.getByRole('combobox', { name: 'Klasa zadań' }).click();
  await page.getByRole('option', { name: /Kod/ }).click();
  await text.fill('dodaj komentarz');
  await text.press('Control+Enter');
  await expect(result).toContainText('Zadanie średnie');
  await expect(result).toContainText('zadanie kodu');
  await expectAccessible(page, 'dobór modelu — podgląd');
});
