// „Naucz Alfę” (P5): okno z palety i z panelu Pamięć — tekst (treść niezaufana, tylko ta rozmowa),
// plik (podgląd z rdzenia), rozmowa w zakresie globalnym, wynik z cofnięciem, błąd zakresu
// projektu; każdy stan okna w każdym stylu i motywie (dostępność i czytelność).
import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, pickOption, waitForChat } from './helpers';

async function open(page: Page, skin = 'classic', theme: 'light' | 'dark' = 'light') {
  await page.setViewportSize({ width: 1600, height: 900 });
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${skin}&theme=${theme}`);
  await waitForChat(page);
}

async function palette(page: Page) {
  await page.keyboard.press('Control+k');
  await expect(page.locator('dialog[open] input')).toBeFocused();
  await page.keyboard.type('Naucz Alfę');
  await expect(page.locator('dialog [role=option][data-selected]')).toContainText('Naucz Alfę');
  await page.keyboard.press('Enter');
  const dialog = page.getByRole('dialog', { name: 'Naucz Alfę' });
  await expect(dialog).toBeVisible();
  return dialog;
}

const TEXT = 'Hotel Pod Giewontem: zameldowanie od 15:00.\n\nPociąg z Krakowa o 7:12.';

test('tekst z palety: podgląd, zapis jako niezaufany, wpis w panelu Pamięć', async ({ page }) => {
  await open(page);
  const dialog = await palette(page);
  await expect(dialog.getByText(/dane, nie polecenia/)).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Naucz', exact: true })).toBeDisabled();
  await dialog.getByLabel('Tekst do nauki').fill(TEXT);
  await dialog.getByLabel('Tytuł (opcjonalnie)').fill('Wyjazd');
  await expect(dialog.getByTestId('teach-estimate')).toHaveText('Zapiszę 1 fakt.');
  await dialog.getByRole('button', { name: 'Naucz', exact: true }).click();
  const result = dialog.getByTestId('teach-result');
  await expect(result).toContainText('Zapisałam 1 fakt w pamięci: Ta rozmowa.');
  await expect(result).toContainText('Źródło: naucz: wklejony tekst · sha256:');
  await expect(result).toContainText('Oznaczone jako niezaufane');
  await dialog.getByRole('button', { name: 'Pokaż w panelu Pamięć' }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole('tab', { name: 'Pamięć', selected: true })).toBeVisible();
  const item = page
    .getByRole('list', { name: 'Wpisy pamięci' })
    .getByRole('button', { name: /\[Wyjazd\] Hotel Pod Giewontem/ });
  await expect(item).toBeVisible();
  await expect(item.getByText('niezaufane')).toBeVisible();
});

test('plik z panelu Pamięć: wybór, zapis i cofnięcie nauki', async ({ page }) => {
  await open(page);
  await page.keyboard.press('Alt+4');
  await page.getByRole('button', { name: 'Naucz Alfę' }).click();
  const dialog = page.getByRole('dialog', { name: 'Naucz Alfę' });
  await dialog.getByRole('radio', { name: 'Plik' }).click();
  await expect(dialog.getByText('Pliki .txt, .md, .pdf i .docx (do 25 MB).')).toBeVisible();
  await dialog.getByRole('button', { name: 'Wybierz plik…' }).click();
  await expect(dialog.getByTestId('teach-file')).toHaveText('wyjazd-zakopane.md — 105 znaków');
  await expect(dialog.getByRole('combobox', { name: 'Zakres' })).toBeDisabled();
  await dialog.getByRole('button', { name: 'Naucz', exact: true }).click();
  await expect(dialog.getByTestId('teach-result')).toContainText('plik „wyjazd-zakopane.md”');
  await dialog.getByRole('button', { name: 'Cofnij naukę' }).click();
  await expect(dialog.getByText('Cofnięto naukę — usunięte fakty: 1.')).toBeVisible();
  await expect(dialog.getByTestId('teach-file')).toHaveCount(0);
  await dialog.getByRole('button', { name: 'Anuluj' }).click();
  await expect(dialog).toHaveCount(0);
  const list = page.getByRole('list', { name: 'Wpisy pamięci' });
  await expect(list.getByRole('button').first()).toBeVisible();
  await expect(list.getByRole('button', { name: /\[wyjazd-zakopane\.md\]/ })).toHaveCount(0);
});

test('rozmowa: zakres globalny; sesja bez projektu — błąd zakresu projektu', async ({ page }) => {
  await open(page);
  let dialog = await palette(page);
  await dialog.getByRole('radio', { name: 'Ta rozmowa' }).click();
  await expect(dialog.getByText(/bez ukrytych wiadomości i wyników narzędzi/)).toBeVisible();
  await expect(dialog.getByText(/dane, nie polecenia/)).toHaveCount(0);
  await pickOption(dialog.getByRole('combobox', { name: 'Zakres' }), 'global');
  await dialog.getByRole('button', { name: 'Naucz', exact: true }).click();
  await expect(dialog.getByTestId('teach-result')).toContainText(
    'w pamięci: Globalna (wszystkie rozmowy).',
  );
  await expect(dialog.getByText('Oznaczone jako niezaufane')).toHaveCount(0);
  await dialog.getByRole('button', { name: 'Zamknij' }).click();

  await page.getByRole('navigation', { name: /Sesje/ }).getByText('Zakupy na weekend').click();
  dialog = await palette(page);
  await dialog.getByRole('radio', { name: 'Ta rozmowa' }).click();
  await pickOption(dialog.getByRole('combobox', { name: 'Zakres' }), 'project');
  await dialog.getByRole('button', { name: 'Naucz', exact: true }).click();
  await expect(dialog.getByRole('alert')).toHaveText(
    'Ta sesja nie należy do projektu — wybierz inny zakres.',
  );
});

for (const skin of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`okno „Naucz Alfę”: formularz, wynik, błąd — ${skin}, ${theme}`, async ({ page }) => {
      await open(page, skin, theme);
      let dialog = await palette(page);
      await dialog.getByLabel('Tekst do nauki').fill(TEXT);
      await expect(dialog.getByTestId('teach-estimate')).toBeVisible();
      await expectAccessible(page, `naucz formularz ${skin} ${theme}`);
      await expectReadable(page, `naucz formularz ${skin} ${theme}`);
      await dialog.getByRole('button', { name: 'Naucz', exact: true }).click();
      await expect(dialog.getByTestId('teach-result')).toBeVisible();
      await expectAccessible(page, `naucz wynik ${skin} ${theme}`);
      await expectReadable(page, `naucz wynik ${skin} ${theme}`);
      await dialog.getByRole('button', { name: 'Zamknij' }).click();

      await page.getByRole('navigation', { name: /Sesje/ }).getByText('Zakupy na weekend').click();
      dialog = await palette(page);
      await dialog.getByRole('radio', { name: 'Plik' }).click();
      await dialog.getByRole('button', { name: 'Wybierz plik…' }).click();
      await expect(dialog.getByTestId('teach-file')).toBeVisible();
      await expectAccessible(page, `naucz plik ${skin} ${theme}`);
      await expectReadable(page, `naucz plik ${skin} ${theme}`);
      await dialog.getByRole('radio', { name: 'Ta rozmowa' }).click();
      await pickOption(dialog.getByRole('combobox', { name: 'Zakres' }), 'project');
      await dialog.getByRole('button', { name: 'Naucz', exact: true }).click();
      await expect(dialog.getByRole('alert')).toBeVisible();
      await expectAccessible(page, `naucz błąd ${skin} ${theme}`);
      await expectReadable(page, `naucz błąd ${skin} ${theme}`);
    });
  }
}
