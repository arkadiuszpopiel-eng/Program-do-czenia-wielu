import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, waitForChat } from './helpers';

// Ustawienia → Stan połączeń (zgłoszenie właściciela 2026-10-14): jedno okno z tym, co jest
// zalogowane i działa, oraz sprawdzenie konta po „Gotowe” w oknie logowania (atrapa).

async function openSettingsPage(page: Page, name: string) {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto('/');
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name, exact: true })
    .click();
  await expect(page.getByRole('heading', { name, level: 2 })).toBeVisible();
}

test('stan połączeń: konta, mosty z planem, modele, ochrona; „Sprawdź wszystko”', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await openSettingsPage(page, 'Stan połączeń');
  const claude = page.locator('[data-conn="bridge:claude_code"]');
  // Logowanie wykrytych narzędzi sprawdza się samo przy otwarciu strony.
  await expect(claude).toContainText('plan Max');
  await expect(claude).toContainText('Alfa może zlecać mu zadania');
  await expect(claude).toHaveAttribute('data-state', 'ok');
  await expect(page.locator('[data-conn="bridge:codex"]')).toHaveAttribute('data-state', 'off');
  await expect(page.locator('[data-conn="bridge:antigravity"]')).toContainText(
    'nie podaje stanu logowania',
  );
  for (const group of ['Konta API', 'Narzędzia z kontem (mosty CLI)', 'Ochrona i system']) {
    await expect(page.getByRole('heading', { name: group, level: 4 })).toBeVisible();
  }
  await expect(page.locator('[data-conn="system:broker"]')).toBeVisible();
  await expectAccessible(page, 'stan połączeń');

  await page.getByRole('button', { name: 'Sprawdź wszystko' }).click();
  await expect(page.getByRole('status').filter({ hasText: 'działa' }).first()).toBeVisible();
  await expect(claude).toHaveAttribute('data-state', 'ok');

  // „Szczegóły” prowadzą do strony z ustawieniami tej pozycji.
  await claude.getByRole('button', { name: /Szczegóły/ }).click();
  await expect(page.getByRole('heading', { name: 'Mosty CLI', level: 2 })).toBeVisible();
  expect(errors).toEqual([]);
});

test('po „Gotowe” w oknie logowania Alfa sama sprawdza konto i mówi, czy może z niego korzystać', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await openSettingsPage(page, 'Mosty CLI');
  const card = page.locator('li.card').filter({ hasText: 'Claude Code (CLI)' });
  await expect(card).toContainText('Zalogowano · plan Max');
  await card.getByRole('button', { name: 'Zaloguj się' }).click();
  const dialog = page.getByRole('dialog', { name: /Logowanie do Claude Code/ });
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Gotowe' }).click();
  await expect(dialog).toBeHidden();
  const toast = page.getByText(/zalogowano \(plan Max\) — Alfa może zlecać zadania/);
  await expect(toast).toBeVisible();
  await page.getByRole('button', { name: 'Stan połączeń' }).last().click();
  await expect(page.getByRole('heading', { name: 'Stan połączeń', level: 2 })).toBeVisible();

  // Antigravity: ostrzeżenie o regulaminie na karcie, logowanie w tym samym oknie.
  await openSettingsPage(page, 'Mosty CLI');
  const agy = page.locator('li.card').filter({ hasText: 'Antigravity CLI (agy)' });
  await expect(agy.getByRole('note')).toContainText('regulamin Antigravity');
  await agy.getByRole('button', { name: 'Zaloguj się' }).click();
  await expect(page.getByRole('dialog', { name: /Logowanie do Antigravity/ })).toBeVisible();
  expect(errors).toEqual([]);
});
