import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Ustawienia → Zaawansowane: surowy edytor obu plików konfiguracji — sprawdzenie składni z numerem
// wiersza, odmowa klucza Jądra bez utraty treści pliku, zapis, cofnięcie zmian, flaga
// eksperymentalna pod edytorem; każdy styl i motyw.

async function openAdvanced(page: Page, skin = 'classic', theme: 'light' | 'dark' = 'light') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${skin}&theme=${theme}`);
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Zaawansowane' })
    .click();
  await expect(page.getByRole('heading', { name: 'Zaawansowane', level: 2 })).toBeVisible();
  await expect(page.getByTestId('advanced-editor')).toHaveValue(/\[ui\]/);
}

test('sprawdzenie, odmowa Jądra, zapis, cofnięcie, warstwy, flaga', async ({ page }) => {
  await openAdvanced(page);
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  await expect(nav.getByRole('button', { name: 'Zaawansowane' })).not.toContainText('wkrótce');
  const editor = page.getByTestId('advanced-editor');
  const save = page.getByRole('button', { name: 'Zapisz', exact: true });
  await expect(save).toBeDisabled();

  await page.getByRole('button', { name: 'Sprawdź' }).click();
  await expect(page.getByRole('status').filter({ hasText: 'kluczy: 3' })).toBeVisible();

  await editor.fill('[ui]\ntheme = "dark"\nzla linia\n');
  await page.getByRole('button', { name: 'Sprawdź' }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'wiersz 3' })).toBeVisible();

  await editor.fill('[kernel]\nautonomy_max = "L4"\n');
  await save.click();
  await expect(page.getByRole('alert').filter({ hasText: 'Brokera' })).toBeVisible();
  await page.getByRole('button', { name: 'Cofnij zmiany' }).click();
  await expect(editor).toHaveValue(/theme = "auto"/);
  await expect(save).toBeDisabled();

  await editor.fill('[ui]\ntheme = "dark"\n');
  await save.click();
  await expect(page.getByRole('status').filter({ hasText: 'zapisana' })).toBeVisible();
  await expect(save).toBeDisabled();

  await page.getByRole('radio', { name: 'Ten komputer' }).click();
  await expect(editor).toHaveValue(/\[camera\]/);
  await expect(page.getByText('laptop.toml')).toBeVisible();
  await page.getByRole('radio', { name: 'Wspólny' }).click();
  await expect(editor).toHaveValue('[ui]\ntheme = "dark"\n');

  await expect(
    page.getByRole('switch', { name: 'Codzienny zapis parametrów komputera' }),
  ).toBeChecked();
});

for (const skin of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`zaawansowane: ${skin}, ${theme}`, async ({ page }) => {
      await openAdvanced(page, skin, theme);
      await page.getByTestId('advanced-editor').fill('[ui\n');
      await page.getByRole('button', { name: 'Sprawdź' }).click();
      await expect(page.getByRole('alert').filter({ hasText: 'wiersz 1' })).toBeVisible();
      await expectAccessible(page, `zaawansowane ${skin} ${theme}`);
      await expectReadable(page, `zaawansowane ${skin} ${theme}`);
    });
  }
}

test('katalog silnika gier NextGO.Gray: pole tekstowe zapisuje ścieżkę', async ({ page }) => {
  await openAdvanced(page);
  const field = page.getByRole('textbox', { name: 'Katalog silnika gier NextGO.Gray' });
  await expect(field).toHaveValue('');
  await expect(page.getByText('target\\release\\engine-server.exe')).toBeVisible();
  await field.fill('E:\\Gry\\NextGO.Gray');
  await field.press('Tab');
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  await nav.getByRole('button', { name: 'Ogólne' }).click();
  await nav.getByRole('button', { name: 'Zaawansowane' }).click();
  await expect(field).toHaveValue('E:\\Gry\\NextGO.Gray');
  await expectAccessible(page, 'zaawansowane: katalog silnika');
});
