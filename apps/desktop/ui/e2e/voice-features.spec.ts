import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

async function settings(page: Page, section: string) {
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  if (!(await nav.isVisible())) await page.keyboard.press('Control+,');
  await nav.getByRole('button', { name: section }).click();
  await expect(page.getByRole('heading', { name: section, level: 2 })).toBeVisible();
}

for (const theme of ['light', 'dark'] as const) {
  test.describe(`głos F5 — motyw ${theme}`, () => {
    test('panel Głos: test słowa wywoławczego, kreator rejestracji, dyktowanie, czytanie', async ({
      page,
    }) => {
      await page.setViewportSize({ width: 1600, height: 900 });
      await openApp(page, { theme });
      await waitForChat(page);
      await page.keyboard.press('Alt+6');
      await expect(page.getByRole('tab', { name: 'Głos', selected: true })).toBeVisible();
      await expect(page.getByText('Słowa wywoławcze wyłączone.')).toBeVisible();
      await expect(page.getByText(/Nieskalibrowane — brak pomiaru FAR\/FRR/)).toBeVisible();
      await expect(page.getByText('Wymaga klucza', { exact: true })).toBeVisible();
      await expectAccessible(page, `panel głos ${theme}`);

      // Test słowa wywoławczego: bramka właściciela bez profilu odrzuca wykrycie.
      await page.getByRole('button', { name: 'Testuj słowo wywoławcze' }).click();
      await expect(page.getByText(/Odrzucone przez bramkę właściciela: 1/)).toBeVisible();
      await page.getByRole('button', { name: 'Zakończ test' }).click();

      // Kreator: 3 frazy z wskaźnikiem jakości → profil.
      await page.getByRole('button', { name: 'Zarejestruj mój głos' }).click();
      await expect(page.getByText('Przeczytaj na głos:')).toBeVisible();
      for (let i = 0; i < 3; i += 1) {
        await page.getByRole('button', { name: 'Nagraj frazę' }).click();
        await expect(page.getByText(/Nagrywam/)).toBeVisible();
        await page.getByRole('button', { name: 'Zakończ nagranie' }).click();
        await expect(page.getByText(/Ostatnia fraza: dobra/)).toBeVisible();
      }
      await expectAccessible(page, `kreator ${theme}`);
      await page.getByRole('button', { name: 'Zakończ rejestrację' }).click();
      await expect(page.getByText('Głos zarejestrowany (fraz: 3).')).toBeVisible();

      // Dyktowanie z odliczaniem i podglądem.
      await page.getByRole('button', { name: 'Dyktuj za 3 s' }).click();
      await expect(page.getByText(/Przejdź do okna docelowego/)).toBeVisible();
      await expect(page.getByText('Dyktuję do: notepad.exe')).toBeVisible({ timeout: 6_000 });
      await expect(page.getByText('dzień dobry kropka')).toBeVisible();
      await expectAccessible(page, `dyktowanie ${theme}`);
      await page.getByRole('button', { name: 'Zakończ dyktowanie' }).click();
      await expect(page.getByText('Dyktowanie wyłączone.')).toBeVisible();

      // Czytanie schowka: sterowanie z klawiatury, Esc zatrzymuje.
      await page.getByRole('button', { name: 'Czytaj schowek' }).click();
      await expect(page.getByRole('toolbar', { name: 'Czytanie na głos' })).toBeVisible();
      const pause = page.getByRole('button', { name: 'Pauza', exact: true });
      await pause.focus();
      await page.keyboard.press('Enter');
      await expect(page.getByRole('button', { name: 'Wznów' })).toBeVisible();
      await expectAccessible(page, `czytanie ${theme}`);
      await page.getByRole('button', { name: 'Wznów' }).click();
      await page.keyboard.press('Escape');
      await expect(page.getByText('Nic nie jest czytane.')).toBeVisible();
    });

    test('Ustawienia → Głos: jawne włączenie z potwierdzeniem ryzyka, profile, tempo', async ({
      page,
    }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await settings(page, 'Głos');
      const enable = page.getByRole('switch', { name: 'Włącz słowa wywoławcze' });
      await expect(enable).toBeVisible();
      await expect(enable).toHaveAttribute('aria-checked', 'false');
      await expectAccessible(page, `ustawienia głos ${theme}`);

      // Bramka właściciela bez profilu — wyłączamy ją, potem włączenie wymaga potwierdzenia ryzyka.
      await page.getByRole('switch', { name: /Tylko mój głos budzi Alfę/ }).click();
      await enable.click();
      const dialog = page.getByRole('alertdialog', {
        name: 'Włączyć nieskalibrowane słowa wywoławcze?',
      });
      await expect(dialog).toBeVisible();
      await expectAccessible(page, `ryzyko ${theme}`);
      await dialog.getByRole('button', { name: 'Włącz na własne ryzyko' }).click();
      await expect(page.getByRole('switch', { name: 'Włącz słowa wywoławcze' })).toHaveAttribute(
        'aria-checked',
        'true',
      );
      await expect(page.getByText('Nasłuchuję słów wywoławczych.').first()).toBeVisible();

      await page.getByLabel('Program').fill('C:\\Windows\\notepad.exe');
      await page.getByRole('button', { name: 'Zapisz profil' }).click();
      await expect(page.getByRole('button', { name: 'Usuń profil notepad.exe' })).toBeVisible();
      await page.getByLabel('Program').fill('notepad');
      await page.getByRole('button', { name: 'Zapisz profil' }).click();
      await expect(page.getByText('Podaj nazwę programu, np. notepad.exe.')).toBeVisible();

      await expect(
        page.getByRole('switch', { name: 'Wymagaj weryfikacji głosu dla akcji ryzykownych' }),
      ).toHaveAttribute('aria-checked', 'true');
      await expect(page.getByRole('slider', { name: /Tempo czytania/ })).toBeVisible();
      await expectAccessible(page, `ustawienia głos po zmianach ${theme}`);
    });
  });
}
