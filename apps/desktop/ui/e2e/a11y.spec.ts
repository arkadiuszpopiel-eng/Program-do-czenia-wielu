import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

for (const theme of ['light', 'dark'] as const) {
  test.describe(`axe — motyw ${theme}`, () => {
    test('rozmowa z wariantami i kartą zatwierdzenia', async ({ page }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await expect(page.getByRole('button', { name: /Otwórz w oknie Brokera/ })).toBeVisible();
      await expectAccessible(page, `rozmowa ${theme}`);
    });

    test('panel prawy: Agentki, Oś czasu, Pliki', async ({ page }) => {
      await page.setViewportSize({ width: 1600, height: 900 });
      await openApp(page, { theme });
      await waitForChat(page);
      for (const [key, name] of [
        ['1', 'Agentki'],
        ['2', 'Oś czasu'],
        ['3', 'Pliki'],
      ] as const) {
        await page.keyboard.press(`Alt+${key}`);
        await expect(page.getByRole('tab', { name, selected: true })).toBeVisible();
        await page.waitForTimeout(150);
        await expectAccessible(page, `panel ${name} ${theme}`);
      }
    });

    test('paleta poleceń i ściągawka', async ({ page }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await page.keyboard.press('Control+k');
      await expect(page.getByRole('dialog', { name: 'Paleta poleceń' })).toBeVisible();
      await expectAccessible(page, `paleta ${theme}`);
      await page.keyboard.press('Escape');
      await page.keyboard.press('Control+/');
      await expect(page.getByRole('dialog', { name: 'Skróty klawiszowe' })).toBeVisible();
      await expectAccessible(page, `ściągawka ${theme}`);
    });

    test('ustawienia: ogólne, dostawcy + kreator, import/eksport, uprawnienia, skróty', async ({
      page,
    }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await page.keyboard.press('Control+,');
      await expect(page.getByRole('heading', { name: 'Ogólne', level: 2 })).toBeVisible();
      await expectAccessible(page, `ustawienia ogólne ${theme}`);
      for (const section of [
        'Modele i dostawcy',
        'Import i eksport',
        'Uprawnienia i bezpieczeństwo',
        'Skróty',
        'Wygląd',
        'Urządzenia',
        'Koszty i limity',
        'Głos',
      ]) {
        await page
          .getByRole('navigation', { name: 'Sekcje ustawień' })
          .getByRole('button', { name: section })
          .click();
        await expect(page.getByRole('heading', { name: section, level: 2 })).toBeVisible();
        await page.waitForTimeout(100);
        await expectAccessible(page, `ustawienia ${section} ${theme}`);
      }
      await page
        .getByRole('navigation', { name: 'Sekcje ustawień' })
        .getByRole('button', { name: 'Modele i dostawcy' })
        .click();
      await page.getByRole('button', { name: 'Dodaj dostawcę' }).click();
      await expect(page.getByRole('heading', { name: 'Dodaj dostawcę' })).toBeVisible();
      await expectAccessible(page, `kreator ${theme}`);
    });

    test('onboarding', async ({ page }) => {
      await openApp(page, { theme, scenario: 'first-run' });
      await expect(page.getByRole('heading', { name: 'Witaj w Alfie' })).toBeVisible();
      for (let step = 0; step < 8; step++) {
        await expectAccessible(page, `onboarding krok ${step + 1} ${theme}`);
        const next = page.getByRole('button', { name: step === 7 ? 'Zaczynamy' : 'Dalej' });
        if (step === 3) await page.getByRole('button', { name: 'Pomiń — dodam później' }).click();
        else await next.click();
      }
      await waitForChat(page);
    });

    for (const scenario of ['offline', 'rate-limited', 'no-keys', 'no-mic', 'disk-low', 'empty']) {
      test(`stan systemowy: ${scenario}`, async ({ page }) => {
        await openApp(page, { theme, scenario });
        await waitForChat(page);
        await page.waitForTimeout(200);
        await expectAccessible(page, `${scenario} ${theme}`);
      });
    }

    test('Szybkie pytanie', async ({ page }) => {
      await openApp(page, { theme, path: '/quick.html' });
      await page.getByLabel('Pytanie', { exact: true }).fill('Ile to 2+2?');
      await page.keyboard.press('Enter');
      await expect(page.getByRole('region', { name: 'Odpowiedź' })).toBeVisible();
      await page.waitForTimeout(1500);
      await expectAccessible(page, `quick ${theme}`);
    });

    test('pigułka głosowa', async ({ page }) => {
      await openApp(page, { theme, path: '/pill.html' });
      await expect(page.getByRole('status')).toContainText('mówi');
      await expectAccessible(page, `pill ${theme}`);
    });
  });
}
