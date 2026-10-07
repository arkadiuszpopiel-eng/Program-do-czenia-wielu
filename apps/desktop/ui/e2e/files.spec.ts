// Załączniki w composerze (spinacz, wklejenie, przeciągnięcie, limity, szacunek tokenów, wysłanie
// z turą), eksport rozmowy do Markdown/HTML (paleta, menu wiadomości) oraz Ustawienia → Import
// i eksport: pełny zakres eksportu i kopie zapasowe z testem przywracania. axe: 0 critical/serious.
import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

async function dropFile(page: Page, name: string, size: number, type: string) {
  await page.getByRole('group', { name: /możesz upuścić tu pliki/ }).evaluate(
    (el, file) => {
      const dt = new DataTransfer();
      dt.items.add(new File(['a'.repeat(file.size)], file.name, { type: file.type }));
      for (const kind of ['dragenter', 'dragover', 'drop']) {
        el.dispatchEvent(
          new DragEvent(kind, { dataTransfer: dt, bubbles: true, cancelable: true }),
        );
      }
    },
    { name, size, type },
  );
}

async function pasteImage(page: Page) {
  await page.locator('#alfa-composer').evaluate((el) => {
    const dt = new DataTransfer();
    dt.items.add(new File(['png'], 'zrzut.png', { type: 'image/png' }));
    el.dispatchEvent(
      new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }),
    );
  });
}

for (const theme of ['light', 'dark'] as const) {
  test.describe(`pliki — motyw ${theme}`, () => {
    test('załączniki: spinacz, wklejenie, upuszczenie, szacunek, wysłanie z turą', async ({
      page,
    }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await page.getByRole('button', { name: 'Dołącz pliki' }).click();
      const chips = page.getByRole('list', { name: /załącznik(i|ów)? wiadomości/ });
      await expect(chips.getByText('raport-q3.pdf')).toBeVisible();
      await pasteImage(page);
      await expect(chips.getByText(/^wklejony-obraz-/)).toBeVisible();
      await dropFile(page, 'notatki.md', 400_000, 'text/markdown');
      await expect(chips.getByText('notatki.md')).toBeVisible();
      await expect(page.getByText(/Duże załączniki: ≈/)).toBeVisible();
      await expect(page.getByRole('status').filter({ hasText: 'Dołączono 1 plik.' })).toHaveCount(
        1,
      );
      await expectAccessible(page, `załączniki ${theme}`);

      await page.getByRole('button', { name: 'Usuń załącznik „raport-q3.pdf”' }).click();
      await expect(chips.getByText('raport-q3.pdf')).toBeHidden();
      const field = page.locator('#alfa-composer');
      await field.fill('Podsumuj załączniki');
      await field.press('Enter');
      const sent = page.getByRole('list', { name: 'Załączniki' }).last();
      await expect(sent.getByText('notatki.md')).toBeVisible();
      await expect(chips).toBeHidden();
      await expectAccessible(page, `wiadomość z załącznikami ${theme}`);
    });

    test('Import i eksport: pełny zakres i kopie zapasowe z testem przywracania', async ({
      page,
    }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await page.keyboard.press('Control+,');
      await page
        .getByRole('navigation', { name: 'Sekcje ustawień' })
        .getByRole('button', { name: 'Import i eksport' })
        .click();
      for (const name of ['Artefakty', 'Logi', 'Nakładka tej maszyny']) {
        await expect(page.getByRole('checkbox', { name, exact: false }).first()).toBeEnabled();
      }
      const backups = page.getByRole('region', { name: 'Kopie zapasowe' });
      await expect(backups.getByText('Nie wybrano katalogu.')).toBeVisible();
      await backups.getByRole('button', { name: 'Wybierz katalog…' }).click();
      await expect(backups.getByText('D:\\Kopie Alfy')).toBeVisible();
      await backups.getByRole('button', { name: 'Utwórz kopię teraz' }).click();
      await expect(page.getByText('Utworzono kopię zapasową.')).toBeVisible();
      await backups
        .getByRole('button', { name: /^Sprawdź kopię/ })
        .first()
        .click();
      await expect(backups.getByText(/jest poprawna/)).toBeVisible();
      await expectAccessible(page, `kopie zapasowe ${theme}`);
      await backups
        .getByRole('button', { name: /^Przywróć z kopii/ })
        .first()
        .click();
      await expect(page.getByText(/^Paczka z /)).toBeVisible();
      // Uchwyt z podglądu (UI nie zna ścieżki) → import; podgląd znika (uchwyt jednorazowy).
      await page.getByRole('button', { name: 'Importuj', exact: true }).click();
      await expect(page.getByText(/^Zaimportowano \d+ element/)).toBeVisible();
      await expect(page.getByText(/^Paczka z /)).toBeHidden();
      // Kopia szyfrowana: „Przywróć…” → prośba o hasło → „Odszyfruj” → podgląd.
      await backups.getByLabel('Hasło kopii (Credential Manager)').fill('długie hasło kopii');
      await backups.getByRole('button', { name: 'Ustaw hasło' }).click();
      await backups.getByRole('button', { name: 'Utwórz kopię teraz' }).click();
      await backups
        .getByRole('button', { name: /^Przywróć z kopii/ })
        .first()
        .click();
      const unlock = page.getByLabel('Paczka jest zaszyfrowana — podaj hasło.');
      await unlock.fill('długie hasło kopii');
      await page.getByRole('button', { name: 'Odszyfruj' }).click();
      await expect(page.getByText(/^Paczka z /)).toBeVisible();
      await expectAccessible(page, `przywracanie kopii ${theme}`);
    });
  });
}

test('eksport rozmowy z palety i z menu wiadomości', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  // Paleta ładuje się leniwie: pisanie przed jej otwarciem trafiłoby do composera (CI).
  await page.keyboard.press('Control+k');
  await expect(page.locator('dialog[open] input')).toBeFocused();
  await page.keyboard.type('Eksportuj rozmowę do Markdown');
  await expect(page.locator('dialog [role=option][data-selected]')).toContainText(
    'Eksportuj rozmowę do Markdown',
  );
  await page.keyboard.press('Enter');
  await expect(page.getByText(/^Zapisano rozmowę: .*\.md$/)).toBeVisible();
  const message = page.getByRole('article', { name: /^Twoja wiadomość/ }).last();
  await message.hover();
  await message.getByRole('button', { name: 'Więcej akcji' }).click();
  await page.getByRole('menuitem', { name: 'Eksportuj wiadomość do HTML' }).click();
  await expect(page.getByText(/^Zapisano rozmowę: .*\.html$/)).toBeVisible();
});
