import { expect, test, type Locator, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Kanwa artefaktów C2.8 (ADR 0009): HTML i SVG w izolowanej ramce (`sandbox` bez skryptów i bez
// `allow-same-origin` — brak dostępu do okna Alfy i `__TAURI_INTERNALS__`), Markdown jako zwykły
// DOM z HTML rdzenia; karty Podgląd / Źródło, kopiowanie, Esc, wąskie okno, panel Pliki; każdy
// stan w każdym stylu i motywie (dostępność i kontrast).

async function open(page: Page, skin = 'classic', theme: 'light' | 'dark' = 'light') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${skin}&theme=${theme}`);
  await waitForChat(page);
}

async function ask(page: Page, text: string) {
  await page.locator('#alfa-composer').fill(text);
  await page.keyboard.press('Enter');
}

/** Blok kodu ostatniej odpowiedzi (np. „html”) — po końcu strumienia ma akcję kanwy. */
function block(page: Page, lang: string): Locator {
  return page.getByRole('figure', { name: `Blok kodu ${lang}`, exact: true }).last();
}

/** Przycisk kanwy bloku — nazwa z tytułem artefaktu („Otwórz w kanwie: Karta projektu”). */
const openButton = (page: Page, lang: string): Locator =>
  block(page, lang).getByRole('button', { name: /^Otwórz w kanwie: / });

async function openBlock(page: Page, lang: string) {
  const button = openButton(page, lang);
  await expect(button).toBeVisible({ timeout: 20_000 });
  await button.click();
}

const canvas = (page: Page) => page.getByTestId('artifact-canvas');

test('HTML: izolowana ramka bez skryptów i bez dostępu do Alfy', async ({ page }) => {
  await open(page);
  // Znacznik w oknie głównym — ramka nie może go odczytać (inne pochodzenie, brak skryptów).
  await page.evaluate(() => {
    const w = window as unknown as Record<string, unknown>;
    w['__TAURI_INTERNALS__'] ??= { probe: true };
  });
  await ask(page, 'Przygotuj do kanwy stronę i rysunek');
  await openBlock(page, 'html');
  const region = canvas(page);
  await expect(openButton(page, 'html')).toHaveAccessibleName('Otwórz w kanwie: Karta projektu');
  await expect(openButton(page, 'svg')).toHaveAccessibleName('Otwórz w kanwie: Rysunek SVG');
  // Otwarcie przenosi fokus na nagłówek kanwy (czytnik ekranu ogłasza region).
  await expect(region.getByRole('heading', { name: 'Karta projektu', level: 2 })).toBeFocused();
  await expect(region.getByRole('tab', { name: 'Podgląd', selected: true })).toBeVisible();
  const frame = region.locator('iframe');
  await expect(frame).toHaveAttribute('sandbox', '');
  // Nazwa ramki także w `aria-label` — `title` zdejmują podpowiedzi Alfy (ui-kit `installTooltips`).
  await expect(frame).toHaveAttribute('aria-label', 'Podgląd: Karta projektu');
  await expect(region.getByText(/izolowanej ramce: bez skryptów/)).toBeVisible();
  const probe = await frame.evaluate((el: HTMLIFrameElement) => {
    let internals: unknown;
    try {
      internals = (el.contentWindow as unknown as Record<string, unknown>)['__TAURI_INTERNALS__'];
    } catch {
      internals = 'zablokowane';
    }
    return {
      tokens: [...el.sandbox],
      sameOrigin: el.contentDocument !== null,
      internals: internals === undefined || internals === 'zablokowane',
      srcdoc: el.srcdoc,
    };
  });
  expect(probe.tokens).toEqual([]);
  expect(probe.sameOrigin).toBe(false);
  expect(probe.internals).toBe(true);
  expect(probe.srcdoc).toContain('<h1>Karta projektu</h1>');
  expect(probe.srcdoc).not.toContain('<script');
  expect(probe.srcdoc).not.toContain('__TAURI_INTERNALS__');
});

test('SVG i Markdown; karta Źródło, kopiowanie, Esc wraca do przycisku', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page);
  await ask(page, 'Przygotuj do kanwy stronę i rysunek');
  await openBlock(page, 'svg');
  const region = canvas(page);
  await expect(region.getByRole('heading', { name: 'Rysunek SVG', level: 2 })).toBeVisible();
  const srcdoc = await region.locator('iframe').evaluate((el: HTMLIFrameElement) => el.srcdoc);
  expect(srcdoc).toContain('<svg viewBox="0 0 120 60"');
  expect(srcdoc).toContain('<rect');

  // Klawiatura: strzałka przełącza kartę i przenosi fokus (wzorzec tablist).
  await region.getByRole('tab', { name: 'Podgląd' }).focus();
  await page.keyboard.press('ArrowRight');
  const sourceTab = region.getByRole('tab', { name: 'Źródło', selected: true });
  await expect(sourceTab).toBeFocused();
  await expect(region.getByRole('tabpanel')).toContainText('<text x="60" y="36"');
  await expect(region.locator('iframe')).toHaveCount(0);
  await region.getByRole('button', { name: 'Kopiuj źródło' }).click();
  await expect(page.getByText('Skopiowano źródło artefaktu.')).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toContain('<svg viewBox');

  await page.keyboard.press('Escape');
  await expect(region).toHaveCount(0);
  await expect(openButton(page, 'svg')).toBeFocused();

  // Markdown z bloku: HTML z renderera (bez ramki), nagłówek jako zwykły DOM.
  await openBlock(page, 'markdown');
  await expect(region.getByRole('tab', { name: 'Podgląd', selected: true })).toBeVisible();
  await expect(region.locator('iframe')).toHaveCount(0);
  await expect(region.getByRole('document').getByText('Notatka ze spotkania')).toBeVisible();
  await expect(region.getByRole('document').getByText('budżet zatwierdzony')).toBeVisible();
  // Zwykły kod nie ma akcji kanwy.
  await expect(page.getByRole('button', { name: 'Otwórz w kanwie' })).toHaveCount(3);
});

test('długa odpowiedź z nagłówkami: „Otwórz odpowiedź w kanwie”', async ({ page }) => {
  await open(page);
  await ask(page, 'Napisz dokument do kanwy');
  const message = page.getByRole('article').filter({ hasText: 'Ryzyka' }).last();
  await expect(message).toContainText('kurs EUR', { timeout: 20_000 });
  await message.hover();
  await message.getByRole('button', { name: 'Otwórz odpowiedź w kanwie' }).click();
  const region = canvas(page);
  await expect(region.getByRole('heading', { name: 'Raport kwartalny', level: 2 })).toBeVisible();
  await expect(region.getByText('Dokument Markdown', { exact: false })).toBeVisible();
  await expect(region.getByRole('document')).toContainText('Przychody');
});

test('panel Pliki: plik Markdown sesji w kanwie', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await open(page);
  await page.keyboard.press('Alt+3');
  await page.getByRole('button', { name: /notatki\.md/ }).click();
  await page.getByRole('button', { name: 'Otwórz w kanwie', exact: true }).click();
  const region = canvas(page);
  await expect(region.getByRole('heading', { name: 'notatki.md', level: 2 })).toBeVisible();
  await expect(region.getByRole('document')).toContainText('sprawdzić kurs EUR');
});

/** Szerokość kolumny rozmowy i kanwy przy otwartej kanwie. */
async function widths(page: Page) {
  const chat = await page.locator('main.center').boundingBox();
  const dock = await page.locator('aside.canvas-dock').boundingBox();
  return { chat: chat?.width ?? 0, dock: dock?.width ?? 0 };
}

for (const width of [1440, 1600, 1100] as const) {
  test(`kanwa obok rozmowy zostawia jej co najmniej 480 px: ${width} px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await open(page);
    // Oba panele otwarte (najciaśniejszy układ): Sesje i panel boczny z kartą Pliki.
    await page.keyboard.press('Alt+3');
    await ask(page, 'Przygotuj do kanwy stronę i rysunek');
    await openBlock(page, 'html');
    await expect(canvas(page)).toBeVisible();
    const size = await widths(page);
    expect(size.chat).toBeGreaterThanOrEqual(480);
    expect(size.dock).toBeGreaterThanOrEqual(320);
    await page.keyboard.press('Escape');
    await expect(canvas(page)).toHaveCount(0);
  });
}

test('zmiana sesji zamyka kanwę', async ({ page }) => {
  await open(page);
  await ask(page, 'Przygotuj do kanwy stronę i rysunek');
  await openBlock(page, 'svg');
  await expect(canvas(page)).toBeVisible();
  await page.getByRole('button', { name: 'Nowa rozmowa' }).first().click();
  await expect(canvas(page)).toHaveCount(0);
});

test('wąskie okno: kanwa jako arkusz z pułapką fokusu, Esc zamyka', async ({ page }) => {
  await page.setViewportSize({ width: 820, height: 700 });
  await open(page);
  await ask(page, 'Przygotuj do kanwy stronę i rysunek');
  await openBlock(page, 'html');
  const dialog = page.getByRole('dialog', { name: 'Kanwa artefaktu' });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Karta projektu', level: 2 })).toBeFocused();
  await expect(dialog.locator('iframe')).toHaveAttribute('sandbox', '');
  const box = await dialog.boundingBox();
  expect(box && box.width <= 820).toBe(true);
  await expectAccessible(page, 'kanwa w wąskim oknie');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
});

test('wąskie okno: kanwa z panelu Pliki zastępuje szufladę (jeden dialog)', async ({ page }) => {
  await page.setViewportSize({ width: 820, height: 700 });
  await open(page);
  await page.keyboard.press('Alt+3');
  const drawer = page.getByRole('dialog', { name: 'Panel boczny' });
  await drawer.getByRole('button', { name: /notatki\.md/ }).click();
  await drawer.getByRole('button', { name: 'Otwórz w kanwie', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Kanwa artefaktu' });
  await expect(dialog.getByRole('heading', { name: 'notatki.md', level: 2 })).toBeFocused();
  await expect(page.getByRole('dialog')).toHaveCount(1);
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole('dialog', { name: 'Panel boczny' })).toBeVisible();
});

for (const skin of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`kanwa — podgląd, źródło, Markdown: ${skin}, ${theme}`, async ({ page }) => {
      await open(page, skin, theme);
      await ask(page, 'Przygotuj do kanwy stronę i rysunek');
      await openBlock(page, 'html');
      const region = canvas(page);
      await expect(region.locator('iframe')).toHaveAttribute('sandbox', '');
      await expectAccessible(page, `kanwa html ${skin} ${theme}`);
      await expectReadable(page, `kanwa html ${skin} ${theme}`);
      await region.getByRole('tab', { name: 'Źródło' }).click();
      await expect(region.getByRole('tab', { name: 'Źródło', selected: true })).toBeVisible();
      await expectAccessible(page, `kanwa zrodlo ${skin} ${theme}`);
      await expectReadable(page, `kanwa zrodlo ${skin} ${theme}`);
      await openBlock(page, 'markdown');
      await expect(region.getByRole('document')).toContainText('Notatka ze spotkania');
      await expectAccessible(page, `kanwa markdown ${skin} ${theme}`);
      await expectReadable(page, `kanwa markdown ${skin} ${theme}`);
    });
  }
}

// Stany graniczne: artefakt ponad limit podglądu (uwaga + ucięte źródło) i błąd renderu z „Ponów”.
for (const skin of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`kanwa — za duży artefakt i błąd podglądu: ${skin}, ${theme}`, async ({ page }) => {
      await open(page, skin, theme);
      await ask(page, 'Pokaż w kanwie stany graniczne');
      await openBlock(page, 'svg');
      const region = canvas(page);
      await expect(region.getByText(/więcej niż limit podglądu/)).toBeVisible();
      await expect(region.locator('iframe')).toHaveCount(0);
      await expectAccessible(page, `kanwa za duza ${skin} ${theme}`);
      await expectReadable(page, `kanwa za duza ${skin} ${theme}`);
      await region.getByRole('tab', { name: 'Źródło' }).click();
      await expect(region.getByText(/^Pokazano pierwsze/)).toBeVisible();
      await expectAccessible(page, `kanwa ucieta ${skin} ${theme}`);
      await expectReadable(page, `kanwa ucieta ${skin} ${theme}`);

      await openBlock(page, 'html');
      await expect(region.getByRole('heading', { name: 'Strona testowa', level: 2 })).toBeVisible();
      const retry = region.getByRole('button', { name: 'Ponów' });
      await expect(retry).toBeVisible();
      await expect(region.locator('iframe')).toHaveCount(0);
      await expectAccessible(page, `kanwa blad ${skin} ${theme}`);
      await expectReadable(page, `kanwa blad ${skin} ${theme}`);
      await retry.click();
      await expect(region.getByRole('button', { name: 'Ponów' })).toBeVisible();
    });
  }
}

// Historia wersji: poprawiona strona o tym samym tytule to wersja 2 tego samego artefaktu —
// przełącznik wersji, karta Zmiany (porównanie wierszy), fokus przy przełączaniu.
async function twoVersions(page: Page) {
  await ask(page, 'Przygotuj do kanwy stronę i rysunek');
  await expect(openButton(page, 'html')).toBeVisible({ timeout: 20_000 });
  await ask(page, 'Popraw w kanwie stronę');
  // Przycisk kanwy pojawia się po zamknięciu bloku — drugi blok html to już wersja 2.
  await expect(
    page
      .getByRole('figure', { name: 'Blok kodu html', exact: true })
      .getByRole('button', { name: /^Otwórz w kanwie: / }),
  ).toHaveCount(2, { timeout: 20_000 });
  await openBlock(page, 'html');
}

test('historia wersji: przełącznik, Zmiany i fokus', async ({ page }) => {
  await open(page);
  await twoVersions(page);
  const region = canvas(page);
  const versions = region.getByRole('group', { name: 'Wersje artefaktu' });
  await expect(versions).toContainText('Wersja 2 z 2 · najnowsza');
  await expect(versions.getByRole('button', { name: 'Następna wersja' })).toBeDisabled();
  await region.getByRole('tab', { name: 'Zmiany' }).click();
  await expect(region.getByText(/wiersze dodane: 2, usunięte: 2/)).toBeVisible();
  const diff = region.getByRole('document', { name: 'Zmiany' });
  await expect(diff).toContainText('Odpowiada: zespół B.');
  await expect(diff).toContainText('Termin oddania: 30 października.');
  // Poprzednia wersja: przycisk „Poprzednia” gaśnie, fokus przechodzi na „Następna”, karta
  // Zmiany znika (pierwsza wersja nie ma z czym się porównać) — kanwa wraca na Podgląd.
  await versions.getByRole('button', { name: 'Poprzednia wersja' }).click();
  await expect(versions).toContainText('Wersja 1 z 2');
  await expect(versions.getByRole('button', { name: 'Następna wersja' })).toBeFocused();
  await expect(region.getByRole('tab', { name: 'Zmiany' })).toHaveCount(0);
  await expect(region.getByRole('tab', { name: 'Podgląd', selected: true })).toBeVisible();
  await region.getByRole('tab', { name: 'Źródło' }).click();
  await expect(region.locator('pre.source')).toContainText('30 października');
  await versions.getByRole('button', { name: 'Następna wersja' }).press('Enter');
  await expect(versions).toContainText('Wersja 2 z 2');
  await expect(region.locator('pre.source')).toContainText('6 listopada');
  // Inny artefakt (rysunek bez tytułu) nie ma historii.
  await openBlock(page, 'svg');
  await expect(region.getByRole('group', { name: 'Wersje artefaktu' })).toHaveCount(0);
});

for (const skin of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`kanwa — wersje i zmiany: ${skin}, ${theme}`, async ({ page }) => {
      await open(page, skin, theme);
      await twoVersions(page);
      const region = canvas(page);
      await expect(region.getByRole('group', { name: 'Wersje artefaktu' })).toBeVisible();
      await expectAccessible(page, `kanwa wersje ${skin} ${theme}`);
      await expectReadable(page, `kanwa wersje ${skin} ${theme}`);
      await region.getByRole('tab', { name: 'Zmiany' }).click();
      await expect(region.getByRole('document', { name: 'Zmiany' })).toBeVisible();
      await expectAccessible(page, `kanwa zmiany ${skin} ${theme}`);
      await expectReadable(page, `kanwa zmiany ${skin} ${theme}`);
    });
  }
}
