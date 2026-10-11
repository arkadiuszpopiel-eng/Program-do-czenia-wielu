import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

async function devices(page: Page) {
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  if (!(await nav.isVisible())) await page.keyboard.press('Control+,');
  await nav.getByRole('button', { name: 'Urządzenia', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Urządzenia', level: 2 })).toBeVisible();
  return page.getByTestId('gpu-live');
}

for (const theme of ['light', 'dark'] as const) {
  test(`Urządzenia: karty graficzne na żywo — motyw ${theme}`, async ({ page }) => {
    await openApp(page, { theme });
    await waitForChat(page);
    const card = await devices(page);
    await expect(card.getByRole('heading', { name: 'Karty graficzne na żywo' })).toBeVisible();
    await expect(card.getByText('NVIDIA GeForce RTX 4050 Laptop GPU')).toBeVisible();
    await expect(card.getByText('pamięć: 3,3 z 6 GB zajęte, 2,7 GB wolne')).toBeVisible();
    const meter = card.getByRole('meter', { name: /Zajęta pamięć karty NVIDIA GeForce RTX 4050/ });
    await expect(meter).toHaveAttribute('aria-valuenow', '56');
    await expect(card.getByText('37 %')).toBeVisible();
    await expect(card.getByText('61 °C')).toBeVisible();
    await expect(card.getByText('48 W')).toBeVisible();
    // Silniki Alfy: 2610 + 180 MB.
    await expect(card.getByText('2,7 GB', { exact: true })).toBeVisible();
    const engine = card.getByRole('listitem').filter({ hasText: 'llama-server.exe' });
    await expect(engine.getByText('Alfa', { exact: true })).toBeVisible();
    await expect(engine.getByText('2610 MB')).toBeVisible();
    const other = card.getByRole('listitem').filter({ hasText: 'explorer.exe' });
    await expect(other.getByText('Alfa', { exact: true })).toHaveCount(0);
    await expect(other.getByText('—')).toBeVisible();
    await expectAccessible(page, `karty na żywo ${theme}`);

    // Wstrzymanie i wznowienie odświeżania z klawiatury.
    const pause = card.getByRole('button', { name: 'Wstrzymaj odświeżanie' });
    await pause.focus();
    await page.keyboard.press('Enter');
    await expect(card.getByRole('button', { name: 'Wznów odświeżanie' })).toBeVisible();
    await card.getByRole('button', { name: 'Odśwież', exact: true }).click();
    await expect(card.getByText('NVIDIA GeForce RTX 4050 Laptop GPU')).toBeVisible();
  });
}

test('Urządzenia: bez pomiaru na żywo jest wyjaśnienie zamiast pustej karty', async ({ page }) => {
  await openApp(page, { scenario: 'no-mic' });
  await waitForChat(page);
  const card = await devices(page);
  await expect(card.getByText(/Odczyt na żywo działa dziś dla kart NVIDIA/)).toBeVisible();
  await expect(card.getByRole('meter')).toHaveCount(0);
});
