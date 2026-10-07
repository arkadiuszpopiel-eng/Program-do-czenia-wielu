import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

async function settings(page: Page, section: string) {
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  if (!(await nav.isVisible())) await page.keyboard.press('Control+,');
  await nav.getByRole('button', { name: section }).click();
  await expect(page.getByRole('heading', { name: section, level: 2 })).toBeVisible();
}

for (const theme of ['light', 'dark'] as const) {
  test.describe(`F8 — motyw ${theme}`, () => {
    test('panel Ekran: kto steruje, zrzut z maską, akcje, „Zatrzymaj” i „Oddaj”', async ({
      page,
    }) => {
      await page.setViewportSize({ width: 1600, height: 900 });
      await openApp(page, { theme });
      await waitForChat(page);
      const indicator = page.getByRole('button', { name: /Delta steruje ekranem/ });
      await expect(indicator).toBeVisible();
      await indicator.click();
      await expect(page.getByRole('tab', { name: 'Ekran', selected: true })).toBeVisible();
      await expect(
        page.getByRole('img', { name: /Ostatni zrzut ekranu agentki Delta/ }),
      ).toBeVisible();
      await expect(page.getByText('Wpisz tekst (24 znaków)')).toBeVisible();
      await expectAccessible(page, `ekran ${theme}`);
      await page.getByRole('button', { name: 'Zatrzymaj sterowanie' }).click();
      await expect(page.getByText('Sterujesz Ty', { exact: true })).toBeVisible();
      await expect(page.getByText('Wpisz tekst — anulowano')).toBeVisible();
      await page.getByRole('button', { name: 'Oddaj sterowanie' }).first().click();
      await expect(page.getByText('Sterujesz Ty', { exact: true })).toHaveCount(0);
      await page.getByRole('button', { name: 'Zezwól na podgląd pulpitu…' }).click();
      await expect(page.getByText(/Otworzono okno Brokera/)).toBeVisible();
    });

    test('terminal: logowanie mostu CLI z karty, wejście z klawiatury, zamknięcie', async ({
      page,
    }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await settings(page, 'Modele i dostawcy');
      const cards = page.getByRole('list', { name: 'Karty zgodności mostów' });
      const claude = cards.getByRole('listitem').filter({ hasText: 'Claude Code (CLI)' });
      await claude.getByRole('button', { name: 'Zaloguj w terminalu' }).click();
      const dialog = page.getByRole('dialog', { name: /Terminal — logowanie do Claude Code/ });
      await expect(dialog).toBeVisible();
      await expect(dialog.getByText('Claude Code (atrapa)')).toBeVisible();
      await expect(dialog.getByText(/Proces \d+ działa/)).toBeVisible();
      await expectAccessible(page, `terminal ${theme}`);
      await page.keyboard.type('/login');
      await page.keyboard.press('Escape');
      await expect(dialog).toBeVisible();
      await expect(dialog.getByText(/> \/login/).first()).toBeVisible();
      await dialog.getByRole('button', { name: 'Zamknij terminal' }).click();
      await expect(dialog).toHaveCount(0);
    });

    test('umiejętności: przegląd diffu z hashem, instalacja, uruchomienie jako zadanie', async ({
      page,
    }) => {
      await page.setViewportSize({ width: 1600, height: 900 });
      await openApp(page, { theme });
      await waitForChat(page);
      await settings(page, 'Umiejętności');
      await page.getByRole('button', { name: 'Przejrzyj: Porządki w Pobranych 1.1.0' }).click();
      const review = page.getByRole('region', { name: /Przegląd: Porządki w Pobranych 1.1.0/ });
      await expect(review.getByText('zastępuje 1.0.0')).toBeVisible();
      await expect(review.getByText(/\+ .*fs_delete/)).toBeVisible();
      await expectAccessible(page, `umiejętności ${theme}`);
      await review.getByRole('button', { name: 'Zainstaluj tę wersję' }).click();
      await expect(page.getByText('Zainstalowano umiejętność.')).toBeVisible();
      await page.getByRole('button', { name: 'Uruchom: Porządki w Pobranych 1.1.0' }).click();
      await page
        .getByLabel('Parametry (JSON)')
        .fill('{"folder": "C:\\\\Users\\\\ala\\\\Downloads"}');
      await page.getByRole('button', { name: 'Zleć zadanie' }).click();
      await expect(page.getByText(/Zlecono zadanie/)).toBeVisible();
    });

    test('Kreator agentek: rozmowa → podgląd z odmianą → test na sucho → zapis', async ({
      page,
    }) => {
      await page.setViewportSize({ width: 1600, height: 900 });
      await openApp(page, { theme });
      await waitForChat(page);
      await settings(page, 'Kreator agentek');
      await page
        .getByLabel('Jaka ma być nowa agentka?')
        .fill('Agentka o imieniu Zofia, która porządkuje pobrane pliki.');
      await page.getByRole('button', { name: 'Zaproponuj szkic' }).click();
      await expect(page.getByLabel('Imię', { exact: true })).toHaveValue('Zofia');
      await page.getByRole('button', { name: 'Pokaż podgląd' }).click();
      const preview = page.getByRole('region', { name: 'Podgląd persony' });
      await expect(preview.getByText('Zofię')).toBeVisible();
      const save = preview.getByRole('button', { name: 'Zapisz agentkę' });
      await expect(save).toBeDisabled();
      await expectAccessible(page, `kreator agentek ${theme}`);
      await preview.getByRole('button', { name: 'Test na sucho' }).click();
      await expect(preview.getByText('Test na sucho zaliczony')).toBeVisible();
      await save.click();
      await expect(page.getByText('Zapisano agentkę Zofia.')).toBeVisible();
      await expect(
        page.getByRole('region', { name: 'Agentki z Kreatora' }).getByText('Zofia'),
      ).toBeVisible();
    });

    test('Replay: podprzebieg Krytyczki i powrót do przebiegu głównego', async ({ page }) => {
      await page.setViewportSize({ width: 1600, height: 900 });
      await openApp(page, { theme });
      await waitForChat(page);
      await page.keyboard.press('Alt+2');
      await page
        .getByRole('group', { name: 'Widok osi czasu' })
        .getByRole('button', { name: 'Replay' })
        .click();
      await page
        .getByRole('list', { name: 'Podprzebiegi' })
        .getByRole('button', { name: /Krytyczka/ })
        .click();
      await expect(page.getByText('Podprzebieg: Krytyczka.')).toBeVisible();
      await expect(page.getByText('Ocena planu')).toBeVisible();
      await expectAccessible(page, `podprzebieg ${theme}`);
      await page.getByRole('button', { name: 'Przejdź do przebiegu głównego' }).click();
      await expect(page.getByRole('list', { name: 'Podprzebiegi' })).toBeVisible();
    });

    test('Zdrowie systemu: naprawa → „Cofnij”, Ulepszacz z digestem, evale', async ({ page }) => {
      await page.setViewportSize({ width: 1600, height: 900 });
      await openApp(page, { theme });
      await waitForChat(page);
      await settings(page, 'Zdrowie systemu');
      await expect(page.getByText('Działa z problemami', { exact: false })).toBeVisible();
      await expect(page.getByText(/wyłącznie w oknie Brokera/)).toBeVisible();
      await expectAccessible(page, `zdrowie ${theme}`);
      await page
        .getByRole('button', {
          name: 'Napraw: Wyłącz trasę „anthropic" do czasu ponownego zalogowania',
        })
        .click();
      await expect(page.getByText('Zlecono naprawę.')).toBeVisible();
      await page.getByRole('button', { name: /^Cofnij: Wyłącz trasę/ }).click();
      await expect(page.getByText('Cofnięto naprawę.')).toBeVisible();
      await page
        .getByRole('button', { name: 'Zatwierdź ten diff: Krótsze odpowiedzi modelu lokalnego' })
        .click();
      await expect(page.getByText('Wdrożono zmianę.')).toBeVisible();
      await expect(page.getByRole('cell', { name: /^F4-agents 1\.0\.0/ })).toBeVisible();
    });
  });
}
