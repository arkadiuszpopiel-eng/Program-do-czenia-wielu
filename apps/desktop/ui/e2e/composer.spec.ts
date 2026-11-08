// Composer rozmowy: nieudane wysłanie nie kasuje szkicu, udane nie przywraca go
// (uwaga przeglądu PR #1: Q-4).
import { expect, test } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

const TEXT = 'Policz budżet na listopad';

test('nieudane wysłanie: treść wraca do pola i jest komunikat błędu', async ({ page }) => {
  await openApp(page, { scenario: 'send-error' });
  await waitForChat(page);
  const field = page.locator('#alfa-composer');
  await field.fill(TEXT);
  await field.press('Enter');
  await expect(field).toHaveValue(TEXT);
  await expect(page.getByText(/Nie udało się zapisać wiadomości/)).toBeVisible();
});

test('udane wysłanie: pole zostaje puste (bez zdublowania treści)', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  const field = page.locator('#alfa-composer');
  await field.fill(TEXT);
  await field.press('Enter');
  await expect(page.locator('main').getByText(TEXT, { exact: true }).first()).toBeVisible();
  await page.waitForTimeout(300);
  await expect(field).toHaveValue('');
});

// Regresja z testu na laptopie (2026-10-07): model pobrany bez silnika — rozmowa pokazywała surowy
// błąd uruchamiania procesu i nie mówiła, co zrobić.
test('model lokalny bez silnika: zrozumiały błąd i przejście do „Modele lokalne i silniki”', async ({
  page,
}) => {
  await openApp(page, { scenario: 'no-engine' });
  await waitForChat(page);
  const field = page.locator('#alfa-composer');
  await field.fill(TEXT);
  await field.press('Enter');
  const alert = page.getByRole('alert').filter({ hasText: 'Brak silnika modeli lokalnych' });
  await expect(alert).toBeVisible();
  await expect(alert).not.toContainText('Błąd dostawcy');
  await expect(alert.getByRole('button', { name: 'Inny model' })).toHaveCount(0);
  await alert.getByRole('button', { name: 'Modele lokalne i silniki' }).click();
  await expect(
    page.getByRole('heading', { level: 2, name: 'Modele lokalne i silniki' }),
  ).toBeVisible();
});

// Właściciel 27.10: „dolny pasek — wyrównanie i symetria”. W wąskim polu (obok panelu prawego,
// telefon) tekst ma cały górny wiersz, a „Wyślij” zostaje przy prawej krawędzi ramki.
for (const width of [1440, 360]) {
  test(`wąskie pole rozmowy: tekst w górnym wierszu, „Wyślij” po prawej — ${width} px`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 800 });
    await openApp(page);
    await waitForChat(page);
    const field = page.locator('#alfa-composer');
    const form = page.locator('form.composer');
    const send = form.getByRole('button', { name: 'Wyślij' });
    const [f, b, s] = await Promise.all([
      field.boundingBox(),
      form.boundingBox(),
      send.boundingBox(),
    ]);
    if (!f || !b || !s) throw new Error('brak pola rozmowy');
    // Podpowiedź w jednej linii, kontrolki pod polem, „Wyślij” przy prawej krawędzi ramki.
    expect(f.height).toBeLessThanOrEqual(40);
    expect(s.y).toBeGreaterThanOrEqual(f.y + f.height - 1);
    expect(b.x + b.width - (s.x + s.width)).toBeLessThanOrEqual(12);
  });
}
