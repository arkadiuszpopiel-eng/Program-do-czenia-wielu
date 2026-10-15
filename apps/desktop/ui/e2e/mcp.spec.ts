import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, pickOption, waitForChat } from './helpers';

// Ustawienia → Serwery MCP: przepisy, formularz (folder, zmienne tajne), lista serwerów, narzędzia
// z zgodą, usuwanie. Stany otwarte (formularz, lista narzędzi, okno usuwania) w każdym stylu i motywie.

async function openMcp(page: Page, skin = 'classic', theme: 'light' | 'dark' = 'light') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${skin}&theme=${theme}`);
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Serwery MCP' })
    .click();
  await expect(page.getByRole('heading', { name: 'Serwery MCP', level: 2 })).toBeVisible();
}

const servers = (page: Page) => page.getByRole('region', { name: 'Twoje serwery' });

test('przepis przeglądarki → zapis → serwer na liście czeka na pierwsze użycie', async ({
  page,
}) => {
  await openMcp(page);
  await expect(page.getByText('Przeglądarka (Playwright)')).toBeVisible();
  await page.getByRole('button', { name: 'Dodaj: Przeglądarka (Playwright)' }).click();
  const form = page.getByRole('region', { name: 'Nowy serwer MCP' });
  await expect(form.getByLabel('Nazwa')).toHaveValue('Przeglądarka (Playwright)');
  await expect(form.getByLabel('Argumenty (jeden w linii)')).toHaveValue(
    '-y\n@playwright/mcp@latest',
  );
  await form.getByRole('button', { name: 'Zapisz' }).click();
  await expect(page.getByText('Zapisano serwer Przeglądarka (Playwright).')).toBeVisible();
  const item = servers(page).getByRole('listitem').filter({ hasText: 'Przeglądarka (Playwright)' });
  await expect(item.getByText('uruchomi się przy pierwszym użyciu')).toBeVisible();
});

test('przepis z folderem i tokenem: zapis zablokowany do uzupełnienia, token jako hasło', async ({
  page,
}) => {
  await openMcp(page);
  await page.getByRole('button', { name: 'Dodaj: Pliki w folderze' }).click();
  const form = page.getByRole('region', { name: 'Nowy serwer MCP' });
  const save = form.getByRole('button', { name: 'Zapisz' });
  await expect(save).toBeDisabled();
  await expect(form.getByText('Wskaż folder.')).toBeVisible();
  await form.getByLabel('Folder').fill('C:\\Users\\Ty\\Projekty');
  await expect(save).toBeEnabled();
  await form.getByRole('button', { name: 'Anuluj' }).click();

  await page.getByRole('button', { name: 'Dodaj: GitHub' }).click();
  const gh = page.getByRole('region', { name: 'Nowy serwer MCP' });
  await expect(gh.getByRole('button', { name: 'Zapisz' })).toBeDisabled();
  const value = gh.getByRole('textbox', { name: 'Wartość' });
  await expect(value).toHaveAttribute('type', 'password');
  await value.fill('ghp_przykladowy');
  await expect(gh.getByRole('button', { name: 'Zapisz' })).toBeEnabled();
  await pickOption(gh.getByRole('combobox', { name: 'Zaufanie' }), 'reviewed');
  await gh.getByRole('button', { name: 'Zapisz' }).click();
  await expect(page.getByText('Zapisano serwer GitHub.')).toBeVisible();
});

test('narzędzia: zgoda, ostrzeżenie o podejrzanym opisie, cofnięcie', async ({ page }) => {
  await openMcp(page);
  const files = servers(page).getByRole('listitem').filter({ hasText: 'Pliki projektu' }).first();
  await files.getByRole('button', { name: 'Sprawdź i pokaż narzędzia' }).click();
  const tools = page.getByLabel('Narzędzia serwera Pliki projektu');
  await expect(tools.getByText('write_file')).toBeVisible();
  await expect(tools.getByText(/próbę przejęcia poleceń/)).toBeVisible();
  await tools.getByRole('button', { name: 'Zezwól: write_file' }).click();
  await expect(page.getByText('Agentka może używać narzędzia write_file.')).toBeVisible();
  await expect(tools.getByRole('button', { name: 'Cofnij zgodę: write_file' })).toBeVisible();
  await expect(files.getByText('narzędzia: 3 z 4 dozwolonych')).toBeVisible();
  await tools.getByRole('button', { name: 'Cofnij zgodę: write_file' }).click();
  await expect(tools.getByRole('button', { name: 'Zezwól: write_file' })).toBeVisible();
  // Wyłączony serwer: bez sprawdzania narzędzi.
  const github = servers(page).getByRole('listitem').filter({ hasText: 'GitHub' }).first();
  await expect(github.getByRole('button', { name: 'Sprawdź i pokaż narzędzia' })).toBeDisabled();
});

test('usuwanie serwera z potwierdzeniem', async ({ page }) => {
  await openMcp(page);
  const github = servers(page).getByRole('listitem').filter({ hasText: 'GitHub' }).first();
  await github.getByRole('button', { name: 'Usuń' }).click();
  const dialog = page.getByRole('alertdialog');
  await expect(dialog.getByText('Usunąć serwer GitHub?')).toBeVisible();
  await dialog.getByRole('button', { name: 'Usuń' }).click();
  await expect(page.getByText('Usunięto serwer GitHub.')).toBeVisible();
  await expect(servers(page).getByText('GitHub')).toHaveCount(0);
});

for (const skin of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`stany otwarte: ${skin}, ${theme} — formularz, narzędzia, okno usuwania`, async ({
      page,
    }) => {
      await openMcp(page, skin, theme);
      const files = servers(page)
        .getByRole('listitem')
        .filter({ hasText: 'Pliki projektu' })
        .first();
      await files.getByRole('button', { name: 'Sprawdź i pokaż narzędzia' }).click();
      await expect(page.getByLabel('Narzędzia serwera Pliki projektu')).toBeVisible();
      await expectAccessible(page, `mcp narzędzia ${skin} ${theme}`);
      await expectReadable(page, `mcp narzędzia ${skin} ${theme}`);
      await page.getByRole('button', { name: 'Dodaj: GitHub' }).click();
      await expect(page.getByRole('region', { name: 'Nowy serwer MCP' })).toBeVisible();
      await expectAccessible(page, `mcp formularz ${skin} ${theme}`);
      await expectReadable(page, `mcp formularz ${skin} ${theme}`);
      await page.getByRole('button', { name: 'Anuluj' }).click();
      await files.getByRole('button', { name: 'Usuń' }).click();
      await expect(page.getByRole('alertdialog')).toBeVisible();
      await expectAccessible(page, `mcp usuwanie ${skin} ${theme}`);
    });
  }
}
