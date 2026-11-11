import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, pickOption, waitForChat } from './helpers';

async function openBuilder(page: Page) {
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Kreator agentek', exact: true })
    .click();
  await expect(page.getByRole('region', { name: 'Agentki z Kreatora' })).toBeVisible();
}

for (const style of ['classic', 'glass', 'next', 'junior']) {
  for (const theme of ['light', 'dark'] as const) {
    test(`Kreator: wspólny Jarvis zachowuje osobny Standard — ${style} ${theme}`, async ({
      page,
    }) => {
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      await page.keyboard.press('Alt+1');
      const alfa = page.locator('.interaction[data-agent="alfa"]');
      const beta = page.locator('.interaction[data-agent="beta"]');
      await pickOption(alfa.getByRole('combobox'), 'standard');
      await openBuilder(page);
      const library = page.getByRole('region', { name: 'Agentki z Kreatora' });
      const common = library.getByRole('combobox', { name: 'Wspólny profil współpracy' });
      await expect(common).toHaveAttribute('data-value', 'standard');
      await expect(library.getByRole('button', { name: 'Sterowanie głosem' })).toHaveCount(0);
      await expect(library).toContainText('Własne agentki dziedziczą ten profil');
      await pickOption(common, 'jarvis');
      await expectAccessible(page, `Kreator Jarvis ${style} ${theme}`);
      const voice = library.getByRole('button', { name: 'Sterowanie głosem' });
      await expect(voice).toBeVisible();
      await voice.click();
      await expect(page.getByRole('tab', { name: 'Głos', selected: true })).toBeVisible();
      await expect(page.getByText('Mikrofon wyłączony.', { exact: true })).toBeVisible();
      await page.keyboard.press('Alt+1');
      await expect(alfa).toHaveAttribute('data-interaction', 'standard');
      await expect(beta).toHaveAttribute('data-interaction', 'jarvis');
      await openBuilder(page);
      await expect(common).toHaveAttribute('data-value', 'jarvis');
      await pickOption(common, 'standard');
      await page.getByRole('button', { name: 'Wróć do rozmowy' }).click();
      await expect(alfa).toHaveAttribute('data-interaction', 'standard');
      await expect(beta).toHaveAttribute('data-interaction', 'standard');
    });
  }
}

test('Kreator: wspólny profil ma etykietę i opis po angielsku', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await openBuilder(page);
  await page.keyboard.press('Control+k');
  await page.locator('dialog[open] input').fill('angielski');
  await expect(page.locator('dialog [role=option][data-selected]')).toContainText('angielski');
  await page.keyboard.press('Enter');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  const common = page.getByRole('combobox', { name: 'Shared interaction profile' });
  await expect(common).toHaveAttribute('data-value', 'standard');
  await expect(page.getByText(/Custom agents inherit this profile/)).toBeVisible();
  await pickOption(common, 'jarvis');
  await expect(common).toHaveAttribute('data-value', 'jarvis');
  await page.getByRole('button', { name: 'Voice controls', exact: true }).click();
  await expect(page.getByRole('tab', { name: 'Voice', selected: true })).toBeVisible();
  await expect(page.getByText('Microphone is off.', { exact: true })).toBeVisible();
  await page.keyboard.press('Control+,');
  await expect(common).toHaveAttribute('data-value', 'jarvis');
});
