// Wprowadzenie („Witaj w Alfie”) po przebudowie układu (0.0.49): boczna szyna kroków z powrotem
// do odwiedzonych, karty wyboru zamiast gołych list, stopka w stałym miejscu i opisy dostawców
// bez surowych znaczników katalogu.
import { expect, test } from '@playwright/test';
import { openApp } from './helpers';

test('szyna kroków: osiem pozycji, postęp i powrót do odwiedzonego kroku', async ({ page }) => {
  await openApp(page, { scenario: 'first-run' });
  const steps = page.getByRole('list', { name: 'Kroki wprowadzenia' });
  await expect(steps.getByRole('listitem')).toHaveCount(8);
  await expect(steps.getByText('opcjonalny')).toHaveCount(3);
  await expect(page.getByText('Krok 1 z 8')).toBeVisible();
  const progress = page.getByRole('progressbar', { name: 'Postęp wprowadzenia' });
  await expect(progress).toHaveAttribute('aria-valuenow', '13');

  await page.getByRole('button', { name: 'Dalej', exact: true }).click();
  await page.getByRole('button', { name: 'Dalej', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Pomiar sprzętu' })).toBeVisible();
  await expect(progress).toHaveAttribute('aria-valuenow', '38');
  // Odwiedzone kroki są przyciskami — kliknięcie wraca; dalsze kroki nie są klikalne.
  await expect(steps.getByRole('button')).toHaveCount(2);
  await steps.getByRole('button', { name: 'Mikrofon' }).click();
  await expect(page.getByRole('heading', { name: 'Sprawdźmy mikrofon' })).toBeVisible();
});

test('stopka nie zmienia położenia między krokami', async ({ page }) => {
  await openApp(page, { scenario: 'first-run' });
  const next = page.getByRole('button', { name: 'Dalej', exact: true });
  const first = await next.boundingBox();
  const tops: number[] = [];
  for (let i = 0; i < 2; i++) {
    await next.click();
    tops.push((await next.boundingBox())?.y ?? -1);
  }
  expect(first).not.toBeNull();
  for (const top of tops) expect(Math.abs(top - (first?.y ?? 0))).toBeLessThanOrEqual(1);
});

test('profil głosu i autonomia to karty wyboru z opisem; zalecany profil jest oznaczony', async ({
  page,
}) => {
  await openApp(page, { scenario: 'first-run' });
  await page.getByRole('button', { name: 'Dalej', exact: true }).click();
  const voices = page.getByRole('radiogroup', { name: 'Profil głosu' });
  await expect(voices.getByRole('radio')).toHaveCount(4);
  const recommended = voices.getByRole('radio', { checked: true });
  await expect(recommended).toContainText('zalecany');
  await voices.getByRole('radio', { name: /Najlżejszy/ }).click();
  await expect(voices.getByRole('radio', { name: /Najlżejszy/ })).toBeChecked();
  // Strzałka przenosi wybór na następną kartę.
  await page.keyboard.press('ArrowRight');
  await expect(voices.getByRole('radio', { name: /Zrównoważony/ })).toBeChecked();
});

test('kreator dostawcy nie pokazuje surowych znaczników katalogu', async ({ page }) => {
  await openApp(page, { scenario: 'first-run' });
  for (let i = 0; i < 3; i++)
    await page.getByRole('button', { name: 'Dalej', exact: true }).click();
  await page.getByRole('button', { name: 'Dodaj teraz' }).click();
  const deepseek = page.getByRole('button', { name: /^DeepSeek/ });
  await expect(deepseek).toContainText('może trenować na Twoich danych');
  await expect(deepseek).toContainText('prawo: Chiny');
  const cards = page.locator('.providers');
  await expect(cards).not.toContainText('unknown');
  await expect(cards).not.toContainText('cn-may-train');
});

test('konto Claude / ChatGPT: logowanie prowadzi krok po kroku i kończy się „Gotowe”', async ({
  page,
}) => {
  await openApp(page, { scenario: 'first-run' });
  for (let i = 0; i < 3; i++)
    await page.getByRole('button', { name: 'Dalej', exact: true }).click();
  await page.getByRole('button', { name: 'Pomiń — dodam później' }).click();
  await page.getByRole('button', { name: 'Dalej', exact: true }).click();
  await page.getByRole('button', { name: 'Później', exact: true }).click();
  await expect(page.getByRole('heading', { name: /Konto Claude albo ChatGPT/ })).toBeVisible();
  // Narzędzie, którego nie ma na komputerze, mówi to wprost i nie daje się kliknąć.
  await expect(page.getByText('nie zainstalowano na tym komputerze')).toBeVisible();
  await expect(page.getByRole('button', { name: /Zaloguj się: Codex/ })).toBeDisabled();
  await page.getByRole('button', { name: /Zaloguj się: Claude/ }).click();
  const dialog = page.getByRole('dialog', { name: 'Logowanie do Claude Code' });
  await expect(
    dialog.getByRole('list', { name: 'Jak się zalogować' }).getByRole('listitem'),
  ).toHaveCount(4);
  await expect(dialog.getByText(/Proces \d+ działa/)).toBeVisible();
  await dialog.getByRole('button', { name: 'Gotowe' }).click();
  await expect(dialog).toHaveCount(0);
  // Po zamknięciu okna logowania kreator jest tam, gdzie był.
  await expect(page.getByText('Krok 7 z 8')).toBeVisible();
});
