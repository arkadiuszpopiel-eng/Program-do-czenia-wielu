import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

async function openSection(page: Page, name: string) {
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name })
    .click();
  await expect(page.getByRole('heading', { name, level: 2 })).toBeVisible();
}

for (const theme of ['light', 'dark'] as const) {
  test.describe(`aktualizacje — motyw ${theme}`, () => {
    test('sprawdź → pobierz → uruchom ponownie → „Co nowego" → przywróć poprzednią', async ({
      page,
    }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await page.keyboard.press('Control+,');
      await openSection(page, 'Aktualizacje');
      await expect(page.getByText('Zainstalowana wersja: 0.1.0-f1')).toBeVisible();
      await expect(page.getByLabel('Instalowanie aktualizacji', { exact: true })).toBeVisible();
      await expectAccessible(page, `aktualizacje ${theme}`);

      await page.getByRole('button', { name: 'Sprawdź teraz' }).click();
      await expect(page.getByText('Dostępna jest wersja 0.2.0.')).toBeVisible();
      await expect(page.getByText('Co nowego · 0.2.0')).toBeVisible();
      await page.getByRole('button', { name: 'Pobierz i przygotuj' }).click();
      await expect(
        page.getByRole('progressbar', { name: 'Pobieranie aktualizacji' }),
      ).toBeVisible();
      await expectAccessible(page, `pobieranie ${theme}`);
      const restart = page.getByRole('button', { name: 'Uruchom ponownie, aby zaktualizować' });
      await expect(restart).toBeVisible({ timeout: 10_000 });
      await expect(
        page.getByText('Wersja 0.2.0 jest gotowa — zacznie działać po ponownym uruchomieniu.'),
      ).toBeVisible();
      await expectAccessible(page, `gotowa ${theme}`);

      await restart.click();
      const news = page.getByRole('dialog', { name: 'Co nowego w wersji 0.2.0' });
      await expect(news).toBeVisible();
      await expect(news.getByText(/Strona „O programie" z licencjami/)).toBeVisible();
      await expectAccessible(page, `co nowego ${theme}`);
      await news.getByRole('button', { name: 'Zamknij' }).click();
      await expect(news).toBeHidden();

      await page.getByRole('button', { name: 'Przywróć poprzednią wersję (0.1.0-f1)' }).click();
      const confirm = page.getByRole('alertdialog', { name: 'Przywrócić poprzednią wersję?' });
      await expect(confirm).toBeVisible();
      await confirm.getByRole('button', { name: 'Przywróć' }).click();
      await expect(
        page.getByText('Przywrócono wersję 0.1.0-f1 — zacznie działać po ponownym uruchomieniu.'),
      ).toBeVisible();
      await expect(
        page.getByRole('button', { name: 'Uruchom ponownie, aby przywrócić' }),
      ).toBeVisible();

      // Baner w rozmowie: przygotowana wersja czeka na ponowne uruchomienie.
      await page.getByRole('button', { name: 'Wróć do rozmowy' }).click();
      await waitForChat(page);
      await expect(
        page.getByText('Poprzednia wersja 0.1.0-f1 jest gotowa do uruchomienia.'),
      ).toBeVisible();
      await expectAccessible(page, `baner ${theme}`);
      await page.getByRole('button', { name: 'Później' }).click();
      await expect(page.getByText('Poprzednia wersja 0.1.0-f1 jest gotowa')).toBeHidden();
    });

    test('O programie: wersja, kanał, licencje z filtrem', async ({ page }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await page.keyboard.press('Control+,');
      await openSection(page, 'O programie');
      await expect(page.getByText('build deweloperski')).toBeVisible();
      await expect(page.getByRole('cell', { name: /reqwest/ })).toBeVisible();
      await expectAccessible(page, `o programie ${theme}`);
      await page.getByLabel('Filtruj licencje').fill('svelte');
      await expect(page.getByText('1 pakiet', { exact: true })).toBeVisible();
      await expect(page.getByRole('cell', { name: /reqwest/ })).toBeHidden();
    });
  });
}

test('offline: sprawdzanie aktualizacji kończy się czytelnym błędem', async ({ page }) => {
  await openApp(page, { scenario: 'offline' });
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await openSection(page, 'Aktualizacje');
  await page.getByRole('button', { name: 'Sprawdź teraz' }).click();
  await expect(page.getByText('Aktualizacja nie powiodła się.')).toBeVisible();
  await expect(page.getByText('Brak połączenia z serwerem wydań.')).toBeVisible();
});
