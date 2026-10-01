import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

// Agentka z narzędziami (Replay, karta „Cofnij”, intencja terminala) i pigułka głosowa —
// na atrapie backendu; axe: 0 naruszeń critical/serious w obu motywach.
for (const theme of ['light', 'dark'] as const) {
  test.describe(`agentki i głos — motyw ${theme}`, () => {
    test('Replay krok po kroku z „Cofnij krok”', async ({ page }) => {
      await page.setViewportSize({ width: 1600, height: 900 });
      await openApp(page, { theme });
      await waitForChat(page);
      await page.keyboard.press('Alt+2');
      await expect(page.getByRole('tab', { name: 'Oś czasu', selected: true })).toBeVisible();
      await page
        .getByRole('group', { name: 'Widok osi czasu' })
        .getByRole('button', { name: 'Replay' })
        .click();
      await expect(
        page.getByText('Przygotuj szkic raportu Q3 z arkuszy przychodów.'),
      ).toBeVisible();
      const steps = page.getByRole('list', { name: 'Kroki przebiegu' });
      await expect(steps.getByRole('listitem')).toHaveCount(4);
      await expect(steps.getByText('czeka na zatwierdzenie')).toBeVisible();
      await expectAccessible(page, `Replay ${theme}`);
      const controls = page.getByRole('group', { name: 'Odtwarzanie krok po kroku' });
      await controls.getByRole('button', { name: 'Od początku' }).click();
      await expect(page.getByText('Krok 1 z 4')).toBeVisible();
      await expect(steps.getByRole('listitem')).toHaveCount(1);
      await controls.getByRole('button', { name: 'Następny krok' }).click();
      await expect(page.getByText('Krok 2 z 4')).toBeVisible();
      await controls.getByRole('button', { name: 'Wszystkie kroki' }).click();
      await expect(steps.getByRole('listitem')).toHaveCount(4);
      await steps.getByRole('button', { name: 'Cofnij krok' }).click();
      await expect(steps.getByText('Cofnięto')).toBeVisible();
    });

    test('zadanie na plikach: karta „Cofnij” i „Uruchom w terminalu”', async ({ page }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await page.locator('#alfa-composer').fill('Delta, uporządkuj pliki w folderze Pobrane.');
      await page.keyboard.press('Enter');
      const card = page
        .getByRole('region', { name: /można cofnąć/ })
        .filter({ hasText: 'Delta: przeniesiono 14 plików' });
      await expect(card).toBeVisible({ timeout: 15_000 });
      await expect(page.getByText('Uruchom w terminalu').first()).toBeVisible();
      await expect(page.getByText('Get-ChildItem -Recurse Archiwum/2026').first()).toBeVisible();
      await expectAccessible(page, `karty agentki ${theme}`);
      await card.getByRole('button', { name: 'Cofnij: Delta: przeniesiono 14 plików' }).click();
      await expect(card).toHaveCount(0);
    });

    test('pigułka głosowa: stan, transkrypt częściowy, przyciski', async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
      await page.goto('/pill.html?mic=hearing&partial=Co%20mam%20jutro');
      const status = page.getByRole('status');
      await expect(status).toHaveText('◉ Słyszy Cię „Co mam jutro”');
      await expect(page.getByRole('button', { name: 'Zatrzymaj mowę' })).toBeVisible();
      const mute = page.getByRole('button', { name: 'Wycisz' });
      await mute.click();
      await expect(page.getByRole('button', { name: 'Włącz mikrofon' })).toHaveAttribute(
        'aria-pressed',
        'true',
      );
      await expectAccessible(page, `pigułka ${theme}`);
    });
  });
}
