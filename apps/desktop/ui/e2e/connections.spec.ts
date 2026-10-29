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
  // Atrapa: wersja wykryta, ale nieprzypięta — wiersz mówi, co zrobić.
  await expect(claude).toContainText('przypnij ją w Mostach CLI');
  await expect(claude).toHaveAttribute('data-state', 'attention');
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
  await expect(claude).toHaveAttribute('data-state', 'attention');

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
  // Kolejne otwarcia i „Gotowe” odświeżają jeden komunikat zamiast dokładać następne
  // (e2e-app 2026-10-29: trzy jednakowe toasty naraz na zrzucie 43b).
  for (let i = 0; i < 2; i += 1) {
    await card.getByRole('button', { name: 'Zaloguj się' }).click();
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Gotowe' }).click();
    await expect(dialog).toBeHidden();
  }
  await expect(toast).toHaveCount(1);
  await expect(toast).toBeVisible();
  await page.getByRole('button', { name: 'Stan połączeń' }).last().click();
  await expect(page.getByRole('heading', { name: 'Stan połączeń', level: 2 })).toBeVisible();

  // Antigravity: ostrzeżenie o regulaminie na karcie, logowanie w tym samym oknie.
  await openSettingsPage(page, 'Mosty CLI');
  const agy = page.locator('li.card').filter({ hasText: 'Antigravity CLI (agy)' });
  await expect(agy.getByRole('note')).toContainText('regulamin Antigravity');
  await expect(agy).toContainText('Gemini CLI Google wyłączył');
  await agy.getByRole('button', { name: 'Zaloguj się' }).click();
  const agyDialog = page.getByRole('dialog', { name: /Logowanie do Google Gemini/ });
  await expect(agyDialog).toBeVisible();
  // Instrukcja krok po kroku i ostrzeżenie przed pisaniem w polu rozmowy z modelem.
  await expect(agyDialog).toContainText('Login with Google');
  await expect(agyDialog).toContainText('nie wpisuj nic w pole „>”');
  // Nagłówek narzędzia z planem → Alfa sama wykrywa zalogowanie (adres e-mail nie jest zapisywany).
  await expect(
    agyDialog.getByRole('status').filter({ hasText: 'plan Google AI Pro' }),
  ).toBeVisible();
  await agyDialog.getByRole('button', { name: 'Gotowe' }).click();
  await expect(page.getByText(/zalogowano \(plan Google AI Pro\)/)).toBeVisible();
  await expect(agy).toContainText('Zalogowano · plan Google AI Pro');
  expect(errors).toEqual([]);
});
