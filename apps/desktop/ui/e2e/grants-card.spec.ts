import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Karta zgód „zawsze w tym zakresie” po przeglądzie 1.11: błąd odczytu listy nie udaje „braku
// zgód” (zostaje ostatnia znana lista i komunikat), a „Odwołaj” zgody, która już wygasła po
// stronie Sędziego, mówi to wprost zamiast „Zgoda odwołana”. Usterki podaje atrapa
// (`window.__alfaGrantsFault`). Każdy stan w każdym stylu i motywie.

const TITLE = 'Zgody „zawsze w tym zakresie”';
const EMPTY = 'Brak zgód — o każde takie działanie Sędzia Cieni zapyta.';
const FAILED = /Nie udało się odczytać zgód od Sędziego Cieni/;
const WEB = 'połączenie z internetem: strona example.com';

async function openGrants(page: Page, skin: string, theme: 'light' | 'dark') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${skin}&theme=${theme}`);
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  const name = 'Uprawnienia i bezpieczeństwo';
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name })
    .click();
  const card = page.getByRole('region', { name: TITLE });
  await expect(card.getByText(WEB)).toBeVisible();
  return card;
}

for (const skin of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`zgody: błąd odczytu zostawia listę — ${skin}, ${theme}`, async ({ page }) => {
      const card = await openGrants(page, skin, theme);
      await page.evaluate(() => {
        window.__alfaGrantsFault = 'list';
        window.dispatchEvent(new Event('focus'));
      });
      await expect(card.getByRole('alert')).toHaveText(FAILED);
      // Ostatnia znana lista zostaje razem z „Odwołaj” — nie „Brak zgód”.
      await expect(card.getByText(WEB)).toBeVisible();
      await expect(card.getByText(EMPTY)).toHaveCount(0);
      await expectAccessible(page, `zgody błąd ${skin} ${theme}`);
      await expectReadable(page, `zgody błąd ${skin} ${theme}`);
      // Łącze wraca: następny odczyt czyści komunikat.
      await page.evaluate(() => {
        window.__alfaGrantsFault = undefined;
        window.dispatchEvent(new Event('focus'));
      });
      await expect(card.getByRole('alert')).toHaveCount(0);
      await expect(card.getByText(WEB)).toBeVisible();
    });
  }
}

test('zgody: „Odwołaj” zgody, której już nie ma, mówi to wprost', async ({ page }) => {
  const card = await openGrants(page, 'classic', 'light');
  await page.evaluate(() => {
    window.__alfaGrantsFault = 'gone';
  });
  await card.getByRole('button', { name: `Odwołaj zgodę: ${WEB}` }).click();
  await expect(page.getByText(/Tej zgody już nie było/)).toBeVisible();
  await expect(page.getByText('Zgoda odwołana — Sędzia Cieni znów zapyta.')).toHaveCount(0);
  await expect(card.getByText(WEB)).toHaveCount(0);
});

test('zgody: pierwszy odczyt nieudany — komunikat zamiast „Brak zgód”', async ({ page }) => {
  await page.addInitScript(() => {
    window.__alfaGrantsFault = 'list';
  });
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto('/');
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Uprawnienia i bezpieczeństwo' })
    .click();
  const card = page.getByRole('region', { name: TITLE });
  await expect(card.getByRole('alert')).toHaveText(FAILED);
  await expect(card.getByText(EMPTY)).toHaveCount(0);
});
