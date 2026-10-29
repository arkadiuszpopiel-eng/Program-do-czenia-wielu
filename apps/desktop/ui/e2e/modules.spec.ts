import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Ustawienia → Moduły: lista z budżetami, moduły Jądra chronione, wyłączenie modułu
// opcjonalnego z banerem „po ponownym uruchomieniu”, wyszukiwanie; każdy styl i motyw.

async function openModules(page: Page, skin = 'classic', theme: 'light' | 'dark' = 'light') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${skin}&theme=${theme}`);
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Moduły' })
    .click();
  await expect(page.getByRole('heading', { name: 'Moduły', level: 2 })).toBeVisible();
}

test('lista, Jądro chronione, wyłączenie opcjonalnego, wyszukiwanie', async ({ page }) => {
  await openModules(page);
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  await expect(nav.getByRole('button', { name: 'Moduły' })).not.toContainText('wkrótce');
  const all = page.getByRole('region', { name: 'Pozostałe moduły' });
  const broker = all.getByRole('listitem').filter({ hasText: 'safety-broker' });
  await expect(broker.getByText('Jądro — chroniony')).toBeVisible();
  await expect(broker.getByRole('switch')).toHaveCount(0);
  const optional = page.getByRole('region', { name: 'Moduły opcjonalne' });
  await expect(optional.getByRole('switch')).toHaveCount(5);
  await expect(page.getByText('Zmiany zadziałają po ponownym uruchomieniu Alfy.')).toHaveCount(0);
  await optional.getByRole('switch', { name: 'Moduł voice-tts włączony' }).click();
  await expect(page.getByText('Zmiany zadziałają po ponownym uruchomieniu Alfy.')).toBeVisible();
  await expect(
    optional.getByRole('switch', { name: 'Moduł voice-tts włączony' }),
  ).not.toBeChecked();
  await optional.getByRole('switch', { name: 'Moduł voice-tts włączony' }).click();
  await expect(page.getByText('Zmiany zadziałają po ponownym uruchomieniu Alfy.')).toHaveCount(0);
  await expect(all.getByText('brak miejsca na dysku')).toHaveCount(0);
  await expect(optional.getByText('brak miejsca na dysku')).toBeVisible();
  await page.getByLabel('Szukaj modułu').fill('voice');
  await expect(optional.getByRole('listitem')).toHaveCount(2);
});

for (const skin of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`moduły: ${skin}, ${theme}`, async ({ page }) => {
      await openModules(page, skin, theme);
      await page
        .getByRole('region', { name: 'Moduły opcjonalne' })
        .getByRole('switch', { name: 'Moduł transfer włączony' })
        .click();
      await expect(
        page.getByText('Zmiany zadziałają po ponownym uruchomieniu Alfy.'),
      ).toBeVisible();
      await expectAccessible(page, `moduły ${skin} ${theme}`);
      await expectReadable(page, `moduły ${skin} ${theme}`);
    });
  }
}
