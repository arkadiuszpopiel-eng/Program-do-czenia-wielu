import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

async function openModels(page: Page) {
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Modele i silniki' })
    .click();
  await expect(
    page.getByRole('heading', { name: 'Pakiety — od 6 (wzorcowy) do 1 (minimalny)' }),
  ).toBeVisible();
}

const card = (page: Page, name: string) =>
  page
    .getByRole('listitem')
    .filter({ has: page.getByRole('heading', { name, level: 4, exact: true }) });

for (const theme of ['light', 'dark'] as const) {
  test(`pakiety 1–6: ocena, dopasowanie, zalecany, elementy i normy — motyw ${theme}`, async ({
    page,
  }) => {
    await openApp(page, { theme });
    await waitForChat(page);
    await openModels(page);
    await expect(page.getByText(/Ten komputer: Radeon 780M/)).toBeVisible();

    const good = card(page, 'Dobry');
    await expect(good.getByRole('img', { name: 'Ocena 4 z 6' })).toBeVisible();
    await expect(good.getByText('Zalecany dla tego komputera')).toBeVisible();
    await expect(good.getByText('Pasuje do tego komputera')).toBeVisible();
    await expect(
      card(page, 'Bardzo dobry').getByText('Na styk — działa z kompromisem'),
    ).toBeVisible();
    const reference = card(page, 'Wzorcowy');
    await expect(reference.getByText('Za słaby sprzęt')).toBeVisible();
    await expect(reference.getByText(/Potrzebna karta graficzna ≥ 8 GB/)).toBeVisible();

    await reference.getByText(/Elementy pakietu/).click();
    await expect(reference.getByText('zapas: procesor')).toBeVisible();
    await reference.getByText(/Jakość i normy/).click();
    await expect(reference.getByText('ISO/IEC 19795-1:2021')).toBeVisible();
    await page.getByText('Jak wybrać pakiet — zalecenia').click();
    await expectAccessible(page, `pakiety ${theme}`);
  });
}

test('pakiet za mocny — dopiero po potwierdzeniu; minimalny pobiera się i czeka na zgodę', async ({
  page,
}) => {
  await openApp(page);
  await waitForChat(page);
  await openModels(page);

  const reference = card(page, 'Wzorcowy');
  await reference.getByRole('button', { name: 'Pobierz pakiet' }).click();
  const warning = page.getByRole('alertdialog', {
    name: 'Pakiet „Wzorcowy” jest za mocny dla tego komputera',
  });
  await expect(warning.getByText(/Zalecany tutaj: 4 · Dobry/)).toBeVisible();
  await warning.getByRole('button', { name: 'Anuluj' }).click();
  await expect(reference.getByRole('status')).toHaveText('Nie pobrano');

  const minimal = card(page, 'Minimalny');
  await minimal.getByRole('button', { name: 'Pobierz pakiet' }).click();
  await expect(
    minimal.getByRole('progressbar', { name: 'Postęp pakietu Minimalny' }),
  ).toBeVisible();
  await expect(minimal.getByRole('status')).toHaveText(/Czeka na Twoją zgodę/, { timeout: 15_000 });
  await expect(minimal.getByText('Zainstalowano 0 z 3 elementów')).toBeVisible();
});

test('„Napraw” elementu — po potwierdzeniu usuwa pliki i pobiera od nowa', async ({ page }) => {
  await openApp(page);
  await waitForChat(page);
  await openModels(page);
  const vad = page.getByRole('listitem').filter({
    has: page.getByRole('heading', { name: 'Silero VAD 6.2.3 (op18, bez If)', level: 4 }),
  });
  await vad.getByRole('button', { name: 'Pobierz' }).click();
  await expect(vad.getByText('Zainstalowano i sprawdzono')).toBeVisible({ timeout: 10_000 });

  await vad.getByRole('button', { name: 'Napraw' }).click();
  const confirm = page.getByRole('alertdialog', {
    name: 'Naprawić „Silero VAD 6.2.3 (op18, bez If)”?',
  });
  await expectAccessible(page, 'naprawa — potwierdzenie');
  await confirm.getByRole('button', { name: 'Napraw' }).click();
  await expect(page.getByText(/Naprawiam „Silero VAD/)).toBeVisible();
  await expect(vad.getByText('Zainstalowano i sprawdzono')).toBeVisible({ timeout: 10_000 });
});
