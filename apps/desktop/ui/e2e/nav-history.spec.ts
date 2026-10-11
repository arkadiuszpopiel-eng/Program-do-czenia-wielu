import { expect, test, type Page } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

const heading = (page: Page, name: string) => page.getByRole('heading', { name, level: 2 });

async function goTo(page: Page, section: string) {
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: section, exact: true })
    .click();
  await expect(heading(page, section)).toBeVisible();
}

/** Boczny przycisk myszy (3 — wstecz, 4 — dalej). */
const sideButton = (page: Page, button: 3 | 4) =>
  page.evaluate(
    (b) =>
      document.body.dispatchEvent(
        new MouseEvent('mouseup', { button: b, bubbles: true, cancelable: true }),
      ),
    button,
  );

test('Wstecz / Dalej: Alt+←, Alt+→ po stronach Ustawień i z powrotem do rozmowy', async ({
  page,
}) => {
  await openApp(page);
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await expect(heading(page, 'Ogólne')).toBeVisible();
  await goTo(page, 'Głos');
  await goTo(page, 'Urządzenia');

  await page.keyboard.press('Alt+ArrowLeft');
  await expect(heading(page, 'Głos')).toBeVisible();
  await page.keyboard.press('Alt+ArrowLeft');
  await expect(heading(page, 'Ogólne')).toBeVisible();
  await page.keyboard.press('Alt+ArrowLeft');
  await expect(page.locator('#alfa-composer')).toBeVisible();
  // Początek historii: nic się nie zmienia, czytnik ekranu dostaje komunikat.
  await page.keyboard.press('Alt+ArrowLeft');
  await expect(page.locator('#alfa-composer')).toBeVisible();
  await expect(
    page.getByRole('status').filter({ hasText: 'Nie ma wcześniejszego miejsca.' }),
  ).toHaveCount(1);

  // Dalej działa także z polem pisania w fokusie.
  await page.locator('#alfa-composer').focus();
  await page.keyboard.press('Alt+ArrowRight');
  await expect(heading(page, 'Ogólne')).toBeVisible();
  await page.keyboard.press('Alt+ArrowRight');
  await expect(heading(page, 'Głos')).toBeVisible();

  // Nowe miejsce ucina gałąź „Dalej” (Urządzenia znikają z historii).
  await goTo(page, 'Wygląd');
  await page.keyboard.press('Alt+ArrowRight');
  await expect(heading(page, 'Wygląd')).toBeVisible();
  await page.keyboard.press('Alt+ArrowLeft');
  await expect(heading(page, 'Głos')).toBeVisible();
});

test('Wstecz / Dalej: boczne przyciski myszy i powrót do poprzedniej sesji', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await openApp(page);
  await waitForChat(page);
  const current = page.locator('[aria-current="page"]').first();
  const first = await current.innerText();
  await page.keyboard.press('Control+2');
  await expect(current).not.toHaveText(first);
  const second = await current.innerText();
  await page.keyboard.press('Control+,');
  await goTo(page, 'Skróty');

  await sideButton(page, 3);
  await expect(heading(page, 'Ogólne')).toBeVisible();
  await sideButton(page, 3);
  await expect(page.locator('#alfa-composer')).toBeVisible();
  await expect(current).toHaveText(second);
  await sideButton(page, 3);
  await expect(current).toHaveText(first);
  await sideButton(page, 4);
  await expect(current).toHaveText(second);
  await sideButton(page, 4);
  await expect(heading(page, 'Ogólne')).toBeVisible();
});

test('Wstecz / Dalej są w tabeli skrótów i w palecie poleceń', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await goTo(page, 'Skróty');
  await expect(page.getByText('Wstecz (poprzednie miejsce)').first()).toBeVisible();
  await expect(page.getByText('Dalej (następne miejsce)').first()).toBeVisible();
  await page.keyboard.press('Control+k');
  const palette = page.getByRole('dialog');
  await palette.getByRole('combobox').fill('wstecz');
  await expect(
    palette.getByRole('option', { name: /Wstecz \(poprzednie miejsce\)/ }),
  ).toBeVisible();
});
