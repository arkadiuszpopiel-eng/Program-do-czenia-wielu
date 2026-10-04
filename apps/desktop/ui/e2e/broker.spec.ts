import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

// Broker poza procesem (ADR 0003) na atrapie backendu: stan w Ustawieniach (usługa / tryb
// przenośny), baner bezpiecznego stanu po zerwaniu łącza, karta „czeka na zatwierdzenie" tylko
// przenosi do okna Brokera (nigdy nie zatwierdza w WebView), kill-switch bez watchdoga.

async function openPermissions(page: Page) {
  await page.keyboard.press('Control+,');
  const name = 'Uprawnienia i bezpieczeństwo';
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name })
    .click();
  await expect(page.getByRole('heading', { name, level: 2 })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Broker (Jądro bezpieczeństwa)' })).toBeVisible();
}

for (const theme of ['light', 'dark'] as const) {
  test.describe(`Broker — motyw ${theme}`, () => {
    test('usługa: karta przenosi do okna Brokera, stan w Ustawieniach', async ({ page }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await expect(page.getByRole('region', { name: 'Stan Brokera' })).toHaveCount(0);
      await expect(page.getByText('Zatwierdzasz tylko w oknie Brokera.')).toBeVisible();
      await page.getByRole('button', { name: /Otwórz w oknie Brokera/ }).click();
      await expect(page.getByText('Przeniesiono do okna Brokera')).toBeVisible();
      // Karta nie ma przycisku zatwierdzenia — tylko przeniesienie.
      await expect(page.getByRole('button', { name: /^Zatwierdź|^Zezwól/ })).toHaveCount(0);
      await openPermissions(page);
      await expect(page.getByText('Usługa Brokera na osobnym koncie Windows')).toBeVisible();
      await expect(page.getByText('· Połączono')).toBeVisible();
      await expect(page.getByText(/Izolacja: pełna/)).toBeVisible();
      await expect(page.getByText(/obsługuje watchdog, poza aplikacją/)).toBeVisible();
      await expectAccessible(page, `Broker usługa ${theme}`);
    });

    test('tryb przenośny: jawnie słabsza izolacja', async ({ page }) => {
      await openApp(page, { theme, scenario: 'broker-portable' });
      await waitForChat(page);
      await openPermissions(page);
      await expect(page.getByText('Tryb przenośny — Broker bez osobnego konta')).toBeVisible();
      await expect(page.getByText(/słabsza izolacja/)).toBeVisible();
      await expect(page.getByText(/Izolacja: słabsza/)).toBeVisible();
      await expect(page.getByText(/Instaluje ją raz administrator/)).toBeVisible();
      await expectAccessible(page, `Broker przenośny ${theme}`);
    });

    test('zerwanie łącza: baner bezpiecznego stanu, karta nie przenosi', async ({ page }) => {
      await openApp(page, { theme, scenario: 'broker-lost' });
      await waitForChat(page);
      const region = page.getByRole('region', { name: 'Stan Brokera' });
      const alert = region.getByRole('alert');
      await expect(alert).toContainText('Połączenie z Brokerem zerwane');
      await expect(alert).toContainText('wszystko, co wymaga zgody, jest odrzucane');
      await expect(region.getByRole('button', { name: 'Ukryj komunikat' })).toHaveCount(0);
      await expect(page.getByText(/Broker jest niedostępny — tej prośby nie da się/)).toBeVisible();
      await page.getByRole('button', { name: /Otwórz w oknie Brokera/ }).click();
      await expect(page.getByText(/okno jest niedostępne — bezpieczny stan/)).toBeVisible();
      await expectAccessible(page, `Broker zerwany ${theme}`);
      await region.getByRole('button', { name: 'Szczegóły' }).click();
      await expect(page.getByText('· Połączenie zerwane — bezpieczny stan')).toBeVisible();
      await expect(page.getByText(/Okno zatwierdzeń: niedostępne/)).toBeVisible();
    });

    test('bez watchdoga: baner awaryjnego kill-switcha do ukrycia', async ({ page }) => {
      await openApp(page, { theme, scenario: 'broker-no-watchdog' });
      await waitForChat(page);
      const region = page.getByRole('region', { name: 'Stan Brokera' });
      await expect(region).toContainText('Watchdog nie działa');
      await expectAccessible(page, `Broker bez watchdoga ${theme}`);
      await region.getByRole('button', { name: 'Ukryj komunikat' }).click();
      await expect(region).toHaveCount(0);
    });
  });
}
