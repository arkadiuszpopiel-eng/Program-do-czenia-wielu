import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

async function triggers(page: Page) {
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  if (!(await nav.isVisible())) await page.keyboard.press('Control+,');
  await nav.getByRole('button', { name: 'Zadania w tle i wyzwalacze', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Zadania w tle i wyzwalacze', level: 2 }),
  ).toBeVisible();
}

for (const theme of ['light', 'dark'] as const) {
  test(`gotowe obowiązki codzienne wypełniają formularz wyzwalacza — motyw ${theme}`, async ({
    page,
  }) => {
    await openApp(page, { theme });
    await waitForChat(page);
    await triggers(page);
    const duties = page.getByRole('group', { name: 'Gotowe obowiązki codzienne' });
    await duties.scrollIntoViewIfNeeded();
    await expect(duties.getByRole('button')).toHaveCount(4);
    await expect(duties.getByText('dni robocze o 8:00 · Alfa')).toBeVisible();
    await expect(duties.getByText('w soboty o 12:00 · Gama')).toBeVisible();
    await expectAccessible(page, `obowiązki ${theme}`);

    // Wybór z klawiatury wypełnia formularz, ale niczego jeszcze nie tworzy.
    const tidy = duties.getByRole('button', { name: 'Porządki w katalogu roboczym' });
    await tidy.focus();
    await page.keyboard.press('Enter');
    await expect(page.getByRole('textbox', { name: 'Nazwa' })).toHaveValue(
      'Porządki w katalogu roboczym',
    );
    await expect(page.getByRole('textbox', { name: 'Co ma zrobić agentka' })).toHaveValue(
      /Niczego nie usuwaj ani nie przenoś/,
    );
    await expect(page.getByRole('status').filter({ hasText: 'Formularz wypełniony' })).toHaveCount(
      1,
    );

    // Po utworzeniu obowiązek jest na liście wyzwalaczy.
    await page.getByRole('button', { name: 'Utwórz wyzwalacz' }).click();
    await expect(page.getByText('Porządki w katalogu roboczym').first()).toBeVisible();
    await expect(page.getByRole('textbox', { name: 'Nazwa' })).toHaveValue('');
  });
}

for (const style of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`ostatni wynik wyzwalacza z raportem agentki — ${style} ${theme}`, async ({ page }) => {
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      await triggers(page);
      const duties = page.getByRole('group', { name: 'Gotowe obowiązki codzienne' });
      await duties.getByRole('button', { name: 'Kontrola kosztów' }).click();
      await page.getByRole('button', { name: 'Utwórz wyzwalacz' }).click();
      const item = page
        .getByRole('list', { name: 'Lista wyzwalaczy' })
        .getByRole('listitem')
        .filter({ hasText: 'Kontrola kosztów' });
      const last = item.getByTestId('trigger-last-run');
      // Przed pierwszym uruchomieniem — jedno zdanie, bez przycisku raportu.
      await expect(last).toHaveText('Jeszcze nie był uruchamiany.');
      await item.getByRole('button', { name: 'Uruchom teraz' }).click();
      // Wynik przychodzi ze zdarzeń zadania, bez przeładowania strony.
      await expect(last).toHaveAttribute('data-tone', 'ok', { timeout: 15_000 });
      await expect(last).toContainText('Udane');
      await expect(last).toContainText('Ostatnio:');
      const toggle = last.getByRole('button', { name: 'Pokaż raport' });
      await toggle.focus();
      await page.keyboard.press('Enter');
      await expect(item.getByText('Gotowe (atrapa).')).toBeVisible();
      await expectAccessible(page, `ostatni wynik ${style} ${theme}`);
      await last.getByRole('button', { name: 'Ukryj raport' }).click();
      await expect(item.getByText('Gotowe (atrapa).')).toHaveCount(0);
    });
  }
}
