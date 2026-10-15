// Karty modeli (AI Act art. 50, zadanie C2.9) w katalogu „Modele lokalne i silniki”: znak
// „Tworzy treść AI” przy modelach rozmowy i głosach, rozwijana karta z pochodzeniem,
// przeznaczeniem, ograniczeniami i licencją — otwarta w każdym stylu i motywie.
import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, openApp, waitForChat } from './helpers';

async function openModels(page: Page) {
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Modele lokalne i silniki' })
    .click();
  await expect(page.getByRole('heading', { name: 'Pozycje katalogu' })).toBeVisible();
}

const row = (page: Page, name: string) =>
  page.getByRole('listitem').filter({ has: page.getByRole('heading', { name, level: 4 }) });

for (const style of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`karta modelu: ${style}, ${theme}`, async ({ page }) => {
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      await openModels(page);
      const bielik = row(page, 'Bielik 1.5B v3.0 Instruct (Q8_0)');
      const vad = row(page, 'Silero VAD 6.2.3 (op18, bez If)');
      await expect(bielik.getByText('Tworzy treść AI')).toBeVisible();
      await expect(vad.getByText('Tworzy treść AI')).toHaveCount(0);

      await bielik.getByText('Karta modelu').click();
      for (const label of ['Pochodzenie', 'Przeznaczenie', 'Ograniczenia', 'Licencja', 'Źródło']) {
        await expect(bielik.getByText(label, { exact: true })).toBeVisible();
      }
      await expect(bielik.getByText(/Alfa oznacza ją jako wygenerowaną/)).toBeVisible();
      await vad.getByText('Karta modelu').click();
      await expect(vad.getByText(/Nie tworzy treści/)).toBeVisible();
      await expectAccessible(page, `karta modelu ${style} ${theme}`);
      await expectReadable(page, `karta modelu ${style} ${theme}`);
    });
  }
}
