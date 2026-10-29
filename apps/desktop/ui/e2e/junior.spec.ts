import { expect, test } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, pickOption, waitForChat } from './helpers';

// Styl „Junior” (etap K, plan 41 — decyzja właściciela 29.10): większy, cieplejszy, zaokrąglony
// interfejs dla dzieci. Po wyborze Alfa proponuje (bez zmiany po cichu) wolniejszą mowę i dźwięki.

test('Junior: wybór w Wyglądzie powiększa interfejs i proponuje wolniejszą mowę z dźwiękami', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto('/');
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Wygląd' })
    .click();
  const html = page.locator('html');
  const zoom = () => html.evaluate((el) => Number(getComputedStyle(el).zoom));
  const before = await zoom();
  await pickOption(page.getByRole('combobox', { name: 'Styl', exact: true }), 'junior');
  await expect(html).toHaveAttribute('data-skin', 'junior');
  await expect.poll(zoom).toBeCloseTo(before * 1.15, 2);
  const offer = page.getByText(/Styl Junior: włączyć też wolniejszą mowę \(85 %\)/);
  await expect(offer).toBeVisible();
  await page.getByRole('button', { name: 'Włącz', exact: true }).click();
  await expect(page.getByText('Mowa w tempie 85 % i dźwięki sygnałowe włączone.')).toBeVisible();
  const saved = await page.evaluate(async () => {
    const w = window as unknown as {
      __alfaClient?: { settings: { get: (k: string) => Promise<unknown> } };
    };
    return w.__alfaClient ? await w.__alfaClient.settings.get('voice.tts_speed') : null;
  });
  if (saved !== null) expect(saved).toBe(85);
  // Powrót do klasycznego: skala wraca, propozycja się nie powtarza.
  await pickOption(page.getByRole('combobox', { name: 'Styl', exact: true }), 'classic');
  await expect(html).not.toHaveAttribute('data-skin', /.+/);
  await expect.poll(zoom).toBeCloseTo(before, 2);
  expect(errors).toEqual([]);
});

for (const theme of ['light', 'dark'] as const) {
  test(`Junior ${theme}: rozmowa, ustawienia i paleta czytelne, axe bez naruszeń`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
    await page.goto(`/?style=junior&theme=${theme}`);
    await waitForChat(page);
    await expect(page.locator('html')).toHaveAttribute('data-skin', 'junior');
    await page.screenshot({ path: `test-results/junior-rozmowa-${theme}.png` });
    await expectAccessible(page, `junior rozmowa ${theme}`);
    await expectReadable(page, `junior rozmowa ${theme}`);
    await page.keyboard.press('Control+k');
    await expect(page.getByRole('dialog').first()).toBeVisible();
    await page.screenshot({ path: `test-results/junior-paleta-${theme}.png` });
    await expectAccessible(page, `junior paleta ${theme}`);
    await page.keyboard.press('Escape');
    await page.keyboard.press('Control+,');
    await expect(page.getByRole('navigation', { name: 'Sekcje ustawień' })).toBeVisible();
    await page.screenshot({ path: `test-results/junior-ustawienia-${theme}.png` });
    await expectAccessible(page, `junior ustawienia ${theme}`);
    await expectReadable(page, `junior ustawienia ${theme}`);
  });
}
