import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

// Odmowa w oknie Sędziego Cieni (plan 50 §4.2, opcja 4 jak w Claude Code): karta w rozmowie
// pozwala powiedzieć agentce, co zrobić inaczej — wskazówka trafia do trwającego zadania.
for (const style of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`wskazówka po odmowie — ${style} ${theme}`, async ({ page }) => {
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      const composer = page.locator('.composer textarea');
      await composer.fill('Uporządkuj folder raportów');
      await composer.press('Enter');
      await expect(page.getByText('Usunięcie 6 duplikatów z folderu Pobrane').last()).toBeVisible();
      await expect(
        page.getByRole('button', { name: 'Powiedz agentce, co zrobić inaczej' }),
      ).toHaveCount(0);
      await page.evaluate(() => window.__alfaResolveApproval?.('denied'));
      const open = page.getByRole('button', { name: 'Powiedz agentce, co zrobić inaczej' });
      await expect(open).toBeVisible();
      await open.click();
      const field = page.getByLabel('Co agentka ma zrobić inaczej?');
      await expect(field).toBeFocused();
      await field.fill('Nie usuwaj — przenieś duplikaty do Archiwum');
      await expectAccessible(page, `wskazówka po odmowie ${style} ${theme}`);
      // Esc zamyka pole bez wysyłania; ponowne otwarcie — puste.
      await field.press('Escape');
      await expect(field).toHaveCount(0);
      await open.click();
      await field.fill('Nie usuwaj — przenieś duplikaty do Archiwum');
      await page.getByRole('button', { name: 'Wyślij agentce' }).click();
      await expect(page.getByText('Agentka dostała wskazówkę')).toBeVisible();
      await expect(field).toHaveCount(0);
    });
  }
}
