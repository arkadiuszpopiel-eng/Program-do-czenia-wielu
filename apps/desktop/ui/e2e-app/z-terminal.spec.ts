// Terminal i okno logowania w PRAWDZIWEJ aplikacji (zgłoszenie właściciela z 2026-10-12: „Alfa
// wywaliła się przy logowaniu przez terminal”). Test otwiera prawdziwy terminal (PowerShell przez
// pseudokonsolę), wpisuje polecenie, zamyka okno i sprawdza, że program działa dalej; potem to samo
// dla okna logowania — bez wpisywania czegokolwiek do narzędzia (logowanie wykonuje człowiek).
import type { Page } from '@playwright/test';
import { expect, shot, test } from './alfa';

async function openSettings(page: Page, section: string): Promise<void> {
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  if (!(await nav.isVisible())) await page.keyboard.press('Control+,');
  await nav.getByRole('button', { name: section, exact: true }).click();
  await expect(page.getByRole('heading', { name: section, level: 2 })).toBeVisible();
}

/** Rdzeń odpowiada — program żyje (to właśnie było wątpliwe w zgłoszeniu). */
async function coreAlive(page: Page): Promise<void> {
  const version = await page.evaluate(() => {
    const ipc = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (cmd: string) => Promise<{ app_version: string }> };
    };
    return ipc.__TAURI_INTERNALS__.invoke('app_bootstrap').then((boot) => boot.app_version);
  });
  expect(version).toMatch(/^\d+\.\d+\.\d+/);
}

test('instancja testowa ma widoczną plakietkę', async ({ alfa }) => {
  const { main } = alfa;
  await expect(main.locator('html')).toHaveAttribute('data-test-instance', '1');
  const badge = await main.evaluate(() => getComputedStyle(document.body, '::after').content);
  expect(badge).toContain('Instancja testowa');
});

test('terminal: prawdziwy PowerShell przyjmuje polecenie, zamknięcie nie wywraca programu', async ({
  alfa,
}) => {
  const { main } = alfa;
  await openSettings(main, 'Komputer');
  await main.getByRole('button', { name: 'Otwórz PowerShell' }).click();
  const dialog = main.getByRole('dialog', { name: 'Terminal — PowerShell' });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText(/Proces \d+ działa/)).toBeVisible({ timeout: 30_000 });
  // Polecenie z wynikiem, którego nie ma w samym wpisanym tekście (7 * 191 = 1337).
  await main.keyboard.type('Write-Output ("wynik-" + (7 * 191))');
  await main.keyboard.press('Enter');
  await expect(dialog.locator('.xterm-rows')).toContainText('wynik-1337', { timeout: 30_000 });
  await shot(alfa, '20-terminal');
  await dialog.getByRole('button', { name: 'Zamknij terminal' }).click();
  await expect(dialog).toHaveCount(0);
  await coreAlive(main);
  const left = await main.evaluate(() => {
    const ipc = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (cmd: string) => Promise<unknown[]> };
    };
    return ipc.__TAURI_INTERNALS__.invoke('terminal_list');
  });
  expect(left, 'po zamknięciu okna nie zostaje otwarty terminal').toEqual([]);
});

test('logowanie do Claude Code: okno z instrukcją otwiera się i zamyka, program działa dalej', async ({
  alfa,
}) => {
  const { main } = alfa;
  await openSettings(main, 'Mosty CLI');
  const card = main
    .getByRole('list', { name: 'Karty zgodności mostów' })
    .getByRole('listitem')
    .filter({ hasText: 'Claude Code (CLI)' });
  await expect(card).toBeVisible({ timeout: 30_000 });
  const login = card.getByRole('button', { name: 'Zaloguj się' });
  // Karta wczytuje się z rdzenia — przycisk dostaje chwilę, zanim uznamy, że narzędzia nie ma.
  await login.waitFor({ timeout: 10_000 }).catch(() => undefined);
  test.skip(
    !(await login.isVisible()) || !(await login.isEnabled()),
    'narzędzie Claude Code nie jest zainstalowane na tym komputerze',
  );
  await login.click();
  const dialog = main.getByRole('dialog', { name: 'Logowanie do Claude Code' });
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByRole('list', { name: 'Jak się zalogować' }).getByRole('listitem'),
  ).toHaveCount(3);
  // Narzędzie albo działa, albo okno mówi wprost, że się nie otworzyło i daje „Spróbuj ponownie”.
  await expect(
    dialog
      .getByText(/Proces \d+ działa/)
      .or(dialog.getByRole('button', { name: 'Spróbuj ponownie' })),
  ).toBeVisible({ timeout: 30_000 });
  await main.waitForTimeout(3_000);
  await shot(alfa, '21-logowanie');
  await coreAlive(main);
  await dialog.getByRole('button', { name: 'Gotowe' }).click();
  await expect(dialog).toHaveCount(0);
  await coreAlive(main);
  await expect(card).toBeVisible();
});
