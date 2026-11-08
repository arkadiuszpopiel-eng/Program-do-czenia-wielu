// Okna poboczne w trybie Tauri (atrapa IPC): Szybkie pytanie i pigułka biorą motyw i język
// z `app_bootstrap` przed pierwszym renderem; pierwsza odpowiedź Szybkiego pytania nie ginie,
// gdy zdarzenia wyprzedzą wynik `quick_ask` (uwagi przeglądu PR #1: Q-2, CP-1, CP-2).
import { expect, test } from '@playwright/test';
import { expectAccessible } from './helpers';
import { mockTauri } from './tauri-mock';

test.beforeEach(async ({ page }) => {
  // Motyw systemu jasny — ciemny może przyjść tylko z ustawienia `ui.theme`.
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
});

test('Szybkie pytanie: motyw i język z bootstrapu', async ({ page }) => {
  await mockTauri(page, { locale: 'en', theme: 'dark' });
  await page.goto('/quick.html');
  const input = page.getByLabel('Question', { exact: true });
  await expect(input).toHaveAttribute('placeholder', 'Ask Alfa…');
  const html = page.locator('html');
  await expect(html).toHaveAttribute('lang', 'en');
  await expect(html).toHaveAttribute('data-theme', 'dark');
  await expectAccessible(page, 'quick en dark');
});

test('Szybkie pytanie: odpowiedź widoczna, choć zdarzenia wyprzedziły wynik quick_ask', async ({
  page,
}) => {
  await mockTauri(page, { locale: 'pl', theme: 'auto', askDelayMs: 300 });
  await page.goto('/quick.html');
  const input = page.getByLabel('Pytanie', { exact: true });
  await input.fill('Ile to 2+2?');
  await input.press('Enter');
  const answer = page.getByRole('region', { name: 'Odpowiedź' });
  await expect(answer).toContainText('Cztery.');
  await expect(page.getByRole('button', { name: 'Otwórz w pełnym oknie' })).toBeVisible();
});

test('pigułka: język i motyw z bootstrapu', async ({ page }) => {
  await mockTauri(page, { locale: 'en', theme: 'dark' });
  await page.goto('/pill.html');
  await expect(page.getByRole('button', { name: 'Stop speech' })).toBeVisible();
  await expect(page.getByRole('main')).toHaveAttribute('aria-label', 'Voice pill');
  const html = page.locator('html');
  await expect(html).toHaveAttribute('lang', 'en');
  await expect(html).toHaveAttribute('data-theme', 'dark');
  await expectAccessible(page, 'pill en dark');
});

test('pigułka (podgląd poza Tauri): ?lang=en ustawia język dokumentu', async ({ page }) => {
  await page.goto('/pill.html?lang=en');
  await expect(page.getByRole('button', { name: 'Stop speech' })).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
});

// Okno ma 640×420 px i przezroczyste tło: karta szersza niż okno traciła prawą krawędź z zaokrągleniem.
test('Szybkie pytanie: karta mieści się w oknie 640×420 także z odpowiedzią', async ({ page }) => {
  await page.setViewportSize({ width: 640, height: 420 });
  await mockTauri(page, { locale: 'pl', theme: 'dark' });
  await page.goto('/quick.html');
  const card = page.getByRole('main');
  const fits = async (label: string) => {
    const box = await card.boundingBox();
    expect(box, label).not.toBeNull();
    if (!box) return;
    expect(box.x, `${label}: lewa`).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width, `${label}: prawa`).toBeLessThanOrEqual(640);
    expect(box.y + box.height, `${label}: dół`).toBeLessThanOrEqual(420);
  };
  await fits('pusta');
  const input = page.getByLabel('Pytanie', { exact: true });
  await input.fill('Ile to 2+2?');
  await input.press('Enter');
  await expect(page.getByRole('region', { name: 'Odpowiedź' })).toContainText('Cztery.');
  await fits('z odpowiedzią');
});
