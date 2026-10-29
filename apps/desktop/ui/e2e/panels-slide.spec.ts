import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

// Panele „w pełni chowane i wysuwane” (właściciel 29.10): Sesje, panel prawy i menu Ustawień chowają
// się w całości (bez paska ikon), przy krawędzi zostaje znacznik, najechanie wysuwa panel nad treść,
// „Przypnij panel” dokuje go z powrotem. Zrzuty — `ALFA_SHOTS=<katalog>` (przegląd wyglądu).
const SHOTS = process.env['ALFA_SHOTS'];
const shot = async (page: Page, name: string) => {
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/${name}.png` });
};

for (const style of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`panele chowane w całości i wysuwane — ${style} ${theme}`, async ({ page }) => {
      // Junior powiększa interfejs ×1,15 — okno o tyle szersze daje ten sam (szeroki) układ.
      await page.setViewportSize({ width: style === 'junior' ? 1840 : 1600, height: 900 });
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      const id = `${style}-${theme}`;
      const sessions = page.getByRole('navigation', { name: 'Sesje' });
      await expect(sessions).toBeVisible();
      await shot(page, `01-start-${id}`);

      // Sesje: przycisk w nagłówku chowa panel całkiem — bez paska ikon, ze znacznikiem przy krawędzi.
      await page.getByRole('button', { name: 'Schowaj panel Sesje (Ctrl+B)' }).click();
      await expect(sessions).toHaveCount(0);
      await expect(page.locator('.rail')).toHaveCount(0);
      const leftTab = page.locator('.edge-tab.left');
      await expect(leftTab).toBeVisible();
      // Najechanie wysuwa listę nad rozmowę; uchwyt pokazuje strzałkę.
      await leftTab.hover();
      const peek = page.getByRole('complementary', { name: 'Sesje' });
      await expect(peek).toBeVisible();
      await shot(page, `02-peek-left-${id}`);
      await expectAccessible(page, `wysunięte Sesje ${id}`);
      // Zjechanie myszą na rozmowę chowa listę.
      await page.mouse.move(800, 450);
      await expect(peek).toHaveCount(0, { timeout: 3_000 });
      // „Przypnij panel” dokuje.
      await leftTab.hover();
      await peek.getByRole('button', { name: 'Przypnij panel' }).click();
      await expect(sessions).toBeVisible();
      await expect(leftTab).toHaveCount(0);

      // Panel prawy: ikona „schowaj” w wierszu kart, Ctrl+\ przywraca.
      await page.getByRole('button', { name: 'Schowaj panel prawy (Ctrl+\\)' }).click();
      const rightTab = page.locator('.edge-tab.right');
      await expect(rightTab).toBeVisible();
      await rightTab.hover();
      const rightPeek = page.getByRole('complementary', { name: 'Panel boczny' });
      await expect(rightPeek).toBeVisible();
      await shot(page, `03-peek-right-${id}`);
      await page.keyboard.press('Escape');
      await expect(rightPeek).toHaveCount(0);
      await page.keyboard.press('Control+Backslash');
      await expect(rightTab).toHaveCount(0);

      // Ustawienia: menu chowa się przyciskiem i Ctrl+B, wysuwa po najechaniu, przypina się.
      await page.keyboard.press('Control+,');
      const nav = page.locator('.settings .nav');
      await expect(nav).toBeVisible();
      await page.getByRole('button', { name: 'Schowaj menu Ustawień (Ctrl+B)' }).click();
      await expect(nav).toHaveCount(0);
      const navTab = page.locator('.settings .edge-tab.right');
      await expect(navTab).toBeVisible();
      await shot(page, `04-settings-hidden-${id}`);
      await navTab.hover();
      const navPeek = page.getByRole('complementary', { name: 'Ustawienia' });
      await expect(navPeek).toBeVisible();
      await shot(page, `05-settings-peek-${id}`);
      await navPeek.getByRole('button', { name: 'Przypnij panel' }).click();
      await expect(nav).toBeVisible();
      await page.keyboard.press('Control+b');
      await expect(nav).toHaveCount(0);
      await page.keyboard.press('Control+b');
      await expect(nav).toBeVisible();
    });
  }
}

test('średnie okno (1280 px): Sesje i panel prawy na zmianę, przycisk w belce nigdy nie jest martwy', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await openApp(page);
  await waitForChat(page);
  const sessions = page.getByRole('navigation', { name: 'Sesje' });
  const toggle = page.getByRole('button', { name: /panel Sesje$/ }).first();
  await page.keyboard.press('Control+Backslash');
  await expect(page.locator('.edge-tab.left')).toBeVisible();
  await expect(sessions).toHaveCount(0);
  await toggle.click();
  await expect(sessions).toBeVisible();
  await expect(page.locator('.edge-tab.right')).toBeVisible();
});

test('przejście panelu jest płynne (bez ograniczania ruchu)', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'no-preference' });
  await page.goto('/?style=glass');
  await waitForChat(page);
  await page.getByRole('button', { name: 'Schowaj panel Sesje (Ctrl+B)' }).click();
  // W trakcie wysuwania panel ma szerokość pośrednią, a na koniec znika.
  const widths: number[] = [];
  for (let i = 0; i < 6; i += 1) {
    const w = await page.evaluate(
      () => document.querySelector('aside.left')?.getBoundingClientRect().width ?? 0,
    );
    widths.push(Math.round(w));
    await page.waitForTimeout(30);
  }
  expect(
    widths.some((w) => w > 0 && w < 260),
    `szerokości: ${widths.join(', ')}`,
  ).toBe(true);
  await expect(page.locator('aside.left')).toHaveCount(0);
});

for (const theme of ['light', 'dark'] as const) {
  test(`tryb pracy bez otwartej rozmowy zmienia się i widać to od razu — ${theme}`, async ({
    page,
  }) => {
    await openApp(page, { scenario: 'empty', theme });
    await waitForChat(page);
    const composer = page.locator('form.composer');
    const chip = composer.getByRole('button', { name: /^Tryb pracy: Automatycznie/ });
    await expect(chip).toBeVisible();
    await chip.click();
    const menu = page.getByRole('menu', { name: 'Tryb pracy' });
    await expect(menu.getByRole('menuitemradio', { checked: true })).toContainText('Automatycznie');
    await menu.getByRole('menuitemradio', { name: /Pytaj o wszystko/ }).click();
    await expect(page.getByText('Tryb pracy: Pytaj o wszystko.')).toBeVisible();
    const lowered = composer.getByRole('button', { name: /^Tryb pracy: Pytaj o wszystko/ });
    await expect(lowered).toBeVisible();
    await lowered.click();
    await expect(menu.getByRole('menuitemradio', { checked: true })).toContainText(
      'Pytaj o wszystko',
    );
  });
}

test('Pełna automatyka: znak w belce (także w Ustawieniach), bursztynowy żeton w pasku pisania', async ({
  page,
}) => {
  await openApp(page, { path: '/?autonomy=L4' });
  await waitForChat(page);
  const badge = page.locator('header.titlebar').getByRole('button', {
    name: /^Włączona Pełna automatyka/,
  });
  await expect(badge).toBeVisible();
  await expect(
    page.locator('form.composer').getByRole('button', { name: /^Tryb pracy: Pełna automatyka/ }),
  ).toBeVisible();
  await page.keyboard.press('Control+,');
  await expect(badge).toBeVisible();
  await badge.click();
  await expect(page.getByRole('menu', { name: 'Tryb pracy' })).toBeVisible();
  await expectAccessible(page, 'znak Pełnej automatyki');
});

// Przegląd 29.10: fokus po wyborze trybu z klawiatury wraca na żeton; menu Ustawień wysunięte w
// chwili wyjścia nie wyjeżdża samo przy następnym otwarciu; w wąskim oknie nie da się go „schować”.
test('tryb z klawiatury oddaje fokus, menu Ustawień nie wyjeżdża samo, wąskie okno bez chowania', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await openApp(page);
  await waitForChat(page);
  const chip = page.locator('form.composer').getByRole('button', { name: /^Tryb pracy: / });
  await chip.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('menu', { name: 'Tryb pracy' })).toBeVisible();
  await page.keyboard.press('2');
  await expect(page.getByRole('menu', { name: 'Tryb pracy' })).toHaveCount(0);
  await expect(page.locator('form.composer [data-level]')).toBeFocused();

  await page.keyboard.press('Control+,');
  await page.getByRole('button', { name: 'Schowaj menu Ustawień (Ctrl+B)' }).click();
  await page.locator('.settings .edge-tab.right').hover();
  const navPeek = page.getByRole('complementary', { name: 'Ustawienia' });
  await expect(navPeek).toBeVisible();
  await page.keyboard.press('Control+,');
  await expect(page.locator('form.composer')).toBeVisible();
  await page.mouse.move(800, 450);
  await page.keyboard.press('Control+,');
  await expect(page.locator('.settings .edge-tab.right')).toBeVisible();
  await expect(navPeek).toHaveCount(0);

  await page.setViewportSize({ width: 600, height: 800 });
  await expect(page.locator('.settings .nav')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Schowaj menu Ustawień (Ctrl+B)' })).toHaveCount(0);
  await page.keyboard.press('Control+b');
  await page.setViewportSize({ width: 1600, height: 900 });
  await expect(page.locator('.settings .edge-tab.right')).toBeVisible();
});
