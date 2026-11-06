import { expect, test, type Page } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

// Alt+M — następny tryb pracy jak Shift+Tab w Claude Code (decyzja właściciela 6.11): Ręcznie →
// Akceptuj edycje → Automatycznie → Plan → Ręcznie; Pełny dostęp nigdy. Działa także w polu pisania
// i w rozmowie z kłódką otwartą „do końca rozmowy” (zmienia wtedy tryb tej rozmowy).

const chip = (page: Page, mode: string) =>
  page.getByRole('button', { name: new RegExp(`^Tryb pracy: ${mode}`) });

declare global {
  interface Window {
    __alfaApproveRaise?: () => void;
  }
}

test('Alt+M zmienia tryb po kolei, bez Pełnego dostępu, także z pola pisania', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await expect(chip(page, 'Automatycznie')).toBeVisible();
  await page.getByRole('textbox').first().focus();
  // Automatycznie → Plan: obniżenie działa od razu.
  await page.keyboard.press('Alt+m');
  await expect(page.getByText('Tryb pracy: Plan.')).toBeVisible();
  await expect(chip(page, 'Plan')).toBeVisible();
  // Plan → Ręcznie: podniesienie czeka na okno Sędziego Cieni.
  await page.keyboard.press('Alt+m');
  await expect(page.getByText('Potwierdź „Ręcznie” w oknie Sędziego Cieni')).toBeVisible();
  await page.evaluate(() => window.__alfaApproveRaise?.());
  await expect(chip(page, 'Ręcznie')).toBeVisible();
  // Ręcznie → Akceptuj edycje → Automatycznie (każde podniesienie przez Sędziego).
  for (const mode of ['Akceptuj edycje', 'Automatycznie']) {
    await page.keyboard.press('Alt+m');
    await expect(page.getByText(`Potwierdź „${mode}” w oknie Sędziego Cieni`)).toBeVisible();
    await page.evaluate(() => window.__alfaApproveRaise?.());
    await expect(chip(page, mode)).toBeVisible();
  }
  await expect(page.getByRole('textbox').first()).toBeFocused();
  // Ściągawka skrótów zna Alt+M.
  await page.keyboard.press('Control+/');
  const sheet = page.getByRole('dialog', { name: 'Skróty klawiszowe' });
  await sheet.getByRole('textbox', { name: 'Szukaj skrótu (nazwa albo klawisze)' }).fill('tryb');
  await expect(sheet.getByText('Następny tryb pracy (bez Pełnego dostępu)')).toBeVisible();
});

test('Alt+M przy kłódce: zamknięta — tylko Plan i Ręcznie; „do końca rozmowy” — każdy tryb tej rozmowy', async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/?child=locked');
  await waitForChat(page);
  await expect(chip(page, 'Ręcznie')).toBeVisible();
  // Kłódka zamknięta: Ręcznie → Plan → Ręcznie, wyższe tryby pominięte.
  await page.keyboard.press('Alt+m');
  await expect(chip(page, 'Plan')).toBeVisible();
  await page.keyboard.press('Alt+m');
  await page.evaluate(() => window.__alfaApproveRaise?.());
  await expect(chip(page, 'Ręcznie')).toBeVisible();

  // Kłódka otwarta „do końca rozmowy” (PIN rodzica 2468 w atrapie).
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Uprawnienia i bezpieczeństwo' })
    .click();
  const card = page.getByRole('region', { name: 'Profil Dziecko' });
  await card.getByLabel('PIN rodzica', { exact: true }).fill('2468');
  await card.getByRole('radio', { name: 'Do końca rozmowy' }).click();
  await card.getByRole('button', { name: 'Otwórz kłódkę' }).click();
  await expect(card.getByRole('status')).toHaveText('Kłódka otwarta do końca tej rozmowy.');
  await page.keyboard.press('Escape');
  await waitForChat(page);

  for (const mode of ['Akceptuj edycje', 'Automatycznie']) {
    await page.keyboard.press('Alt+m');
    await expect(page.getByText(`Potwierdź „${mode}” w oknie Sędziego Cieni`)).toBeVisible();
    await page.evaluate(() => window.__alfaApproveRaise?.());
    await expect(chip(page, mode)).toBeVisible();
  }
  // Automatycznie → Plan (od razu), Pełny dostęp nigdy ze skrótu.
  await page.keyboard.press('Alt+m');
  await expect(chip(page, 'Plan')).toBeVisible();
});
