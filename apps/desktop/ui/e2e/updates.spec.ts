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

for (const style of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`historia zmian: wagi, filtr i zwijanie — ${style} ${theme}`, async ({ page }) => {
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      await page.keyboard.press('Control+,');
      await openSection(page, 'Aktualizacje');
      const history = page.getByRole('region', { name: 'Historia zmian' });
      const version = (v: string) => history.getByRole('button', { name: `Wersja ${v}` });
      // Najnowsza wersja jest rozwinięta, starsze zwinięte.
      await expect(version('0.1.0-f1')).toHaveAttribute('aria-expanded', 'true');
      await expect(version('0.0.9')).toHaveAttribute('aria-expanded', 'false');
      await expect(
        history.getByText('Kopia zapasowa nie powstaje w wersji nieszyfrowanej'),
      ).toBeVisible();
      // Waga jest podana słowem, nie tylko kolorem; liczniki przy wersji mają opis dla czytnika.
      await expect(history.getByText('krytyczna', { exact: true })).toBeVisible();
      await expect(version('0.1.0-f1')).toContainText('krytyczna: 1');
      // Filtr „Krytyczne” zostawia jedną wersję i jedną pozycję.
      await history.getByRole('button', { name: /^Krytyczne/ }).click();
      await expect(history.getByRole('button', { name: /^Krytyczne/ })).toHaveAttribute(
        'aria-pressed',
        'true',
      );
      await expect(history.getByRole('button', { name: /^Wersja / })).toHaveCount(1);
      await expect(history.getByText('zwykła', { exact: true })).toHaveCount(0);
      await history.getByRole('button', { name: /^Wszystkie/ }).click();
      // Zwijanie i rozwijanie klawiaturą oraz wszystkiego naraz.
      await version('0.0.9').focus();
      await page.keyboard.press('Enter');
      await expect(history.getByText('Głośność mowy: głośność głosu agentek')).toBeVisible();
      await history.getByRole('button', { name: 'Rozwiń wszystkie', exact: true }).click();
      await expect(history.getByText('Pierwszy stan uruchamialny.')).toBeVisible();
      await expectAccessible(page, `historia zmian ${style} ${theme}`);
      await history.getByRole('button', { name: 'Zwiń wszystkie', exact: true }).click();
      await expect(history.getByText('Kopia zapasowa nie powstaje')).toHaveCount(0);
      await expect(version('0.1.0-f1')).toHaveAttribute('aria-expanded', 'false');
    });
  }
}

for (const style of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`historia zmian: dziesięć ostatnich i zakładka „Starsze” — ${style} ${theme}`, async ({
      page,
    }) => {
      await openApp(page, { theme, path: `/?style=${style}&history=long` });
      await waitForChat(page);
      await page.keyboard.press('Control+,');
      await openSection(page, 'Aktualizacje');
      const history = page.getByRole('region', { name: 'Historia zmian' });
      const versions = history.getByRole('button', { name: /^Wersja / });
      const recent = history.getByRole('tab', { name: /^Ostatnie/ });
      const older = history.getByRole('tab', { name: /^Starsze/ });
      // Główny widok: dziesięć wersji, najnowsza rozwinięta; reszta czeka w zakładce.
      await expect(recent).toHaveAttribute('aria-selected', 'true');
      await expect(recent).toContainText('10');
      await expect(older).toContainText('7');
      await expect(versions).toHaveCount(10);
      await expect(versions.first()).toHaveAttribute('aria-expanded', 'true');
      await expectAccessible(page, `historia: ostatnie ${style} ${theme}`);
      // Przycisk pod listą prowadzi do starszych i oddaje fokus zakładce.
      await history.getByRole('button', { name: 'Starsze wersje (7)' }).click();
      await expect(older).toHaveAttribute('aria-selected', 'true');
      await expect(older).toBeFocused();
      await expect(versions).toHaveCount(7);
      await expect(history.getByRole('button', { name: /^Wersja 0\.0\.7-p1 / })).toBeVisible();
      await history.getByRole('button', { name: 'Rozwiń wszystkie', exact: true }).click();
      await expect(history.getByText('Poprawka numer 1 z okresu prób.')).toBeVisible();
      await expectAccessible(page, `historia: starsze ${style} ${theme}`);
      // Filtr działa w obrębie zakładki, a liczniki zakładek idą za filtrem.
      await history.getByRole('button', { name: /^Poważne/ }).click();
      await expect(versions).toHaveCount(1);
      await expect(recent).toContainText('3');
      await history.getByRole('button', { name: /^Krytyczne/ }).click();
      await expect(
        history.getByText('W tym zakresie żadna wersja nie ma zmian o tej wadze.'),
      ).toBeVisible();
      await history.getByRole('button', { name: /^Wszystkie/ }).click();
      // Strzałki przełączają zakładki; powrót także przyciskiem pod listą.
      await older.focus();
      await page.keyboard.press('ArrowLeft');
      await expect(recent).toHaveAttribute('aria-selected', 'true');
      await expect(recent).toBeFocused();
      await expect(versions).toHaveCount(10);
      await page.keyboard.press('ArrowRight');
      await history.getByRole('button', { name: 'Wróć do ostatnich wersji' }).click();
      await expect(recent).toHaveAttribute('aria-selected', 'true');
    });
  }
}
