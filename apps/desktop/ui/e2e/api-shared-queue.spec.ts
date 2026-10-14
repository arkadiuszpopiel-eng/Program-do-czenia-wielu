import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, waitForChat } from './helpers';

// Ustawienia → Kolejki modeli API → „Wszystkie konta” (zgłoszenie właściciela 2026-10-14): obok
// kolejek pojedynczych kont jedna wspólna kolejka z modelami z różnych kont.

async function openShared(page: Page) {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto('/');
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Kolejki modeli API' })
    .click();
  await page.getByRole('tab', { name: /Wszystkie konta/ }).click();
}

const tiles = (page: Page) => page.locator('ol.tiles > li');

test('wspólna kolejka: modele z dwóch kont, kolejność, złożenie z kolejek kont i czyszczenie', async ({
  page,
}) => {
  await openShared(page);
  await expect(page.getByRole('tab', { name: /Wszystkie konta/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await expect(page.getByText('Brak wspólnej kolejki.')).toBeVisible();
  await expectAccessible(page, 'wspólna kolejka — pusta');

  // Model z konta Anthropic, potem z konta OpenAI.
  const account = page.getByRole('combobox', { name: 'Konto' });
  await account.click();
  await page.getByRole('option', { name: /Anthropic/ }).click();
  await page.getByRole('button', { name: 'Dodaj do kolejki: claude-opus-5-5' }).click();
  await expect(tiles(page)).toHaveCount(1);
  await account.click();
  await page.getByRole('option', { name: /OpenAI/ }).click();
  await page.getByRole('button', { name: 'Dodaj do kolejki: gpt-6-sol' }).click();
  await expect(tiles(page)).toHaveCount(2);
  await expect(tiles(page).nth(0).locator('.name')).toHaveText('Claude Opus 5.5');
  await expect(tiles(page).nth(0).locator('.owner')).toHaveText('Anthropic (Claude)');
  await expect(tiles(page).nth(0).locator('code')).toHaveText('claude-opus-5-5');
  await expect(tiles(page).nth(1).locator('.owner')).toHaveText('OpenAI');
  await expect(tiles(page).nth(1).locator('code')).toHaveText('gpt-6-sol');
  await expect(page.getByRole('tab', { name: /Rozmowa/ })).toContainText('2');
  // Model już w kolejce nie daje się dodać drugi raz.
  await expect(page.getByRole('button', { name: 'Dodaj do kolejki: gpt-6-sol' })).toHaveCount(0);
  await expect(page.locator('li.model').filter({ hasText: 'gpt-6-sol' }).first()).toContainText(
    'w kolejce',
  );
  await expectAccessible(page, 'wspólna kolejka — dwa konta');

  // Usunięcie pozycji.
  await page
    .getByRole('button', { name: 'Usuń z kolejki: Anthropic (Claude) · claude-opus-5-5' })
    .click();
  await expect(tiles(page)).toHaveCount(1);

  // Złożenie z kolejek kont: konto OpenAI ma kolejkę rozmowy w atrapie? Jeśli nie — komunikat.
  await page.getByRole('button', { name: 'Złóż z kolejek kont' }).click();
  await expect(
    page
      .getByText('Wspólna kolejka „Rozmowa” złożona z kolejek kont.')
      .or(page.getByText('Żadne konto nie ma kolejki dla tej klasy zadań.')),
  ).toBeVisible();

  await page.getByRole('button', { name: 'Wyczyść kolejkę' }).click();
  await expect(page.getByText('Brak wspólnej kolejki.')).toBeVisible();

  // Zakładka konta działa jak dotąd.
  await page.getByRole('tab', { name: /Praca — OpenAI/ }).click();
  await expect(
    page.getByRole('heading', { name: 'Kolejki modeli', exact: true, level: 3 }),
  ).toBeVisible();
});
