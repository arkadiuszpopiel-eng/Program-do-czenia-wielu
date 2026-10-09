import { expect, test, type Locator, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Dodawanie klucza API (zgłoszenie właściciela 2026-10-09: „401 Klucz odrzucony” bez wyjaśnienia).
// Kreator prowadzi krok po kroku, sprząta wklejony klucz, a wynik testu ma osobne wyjaśnienie,
// kroki naprawy i właściwe przyciski dla każdej przyczyny. W atrapie wynik zależy od słowa w kluczu.

/** Zmyślony klucz o długości prawdziwego: przedrostek + znacznik wyniku + wypełnienie. */
const fakeKey = (prefix: string, mark: string) => `${prefix}${mark}-${'XXXX'.repeat(8)}`;
const verdictIn = (scope: Page | Locator) => scope.locator('[data-verdict]');

async function openProviders(page: Page, query = '') {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto(`/${query}`);
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Konta i klucze API' })
    .click();
}

async function pickProvider(page: Page, provider: RegExp) {
  await page.getByRole('button', { name: 'Dodaj dostawcę' }).click();
  await page.getByRole('button', { name: provider }).first().click();
  await expect(page.getByLabel('Klucz API')).toBeFocused();
}

async function openWizard(page: Page, provider: RegExp = /^OpenAI/, query = '') {
  await openProviders(page, query);
  await pickProvider(page, provider);
}

/** Wkleja klucz, wysyła i czeka na wynik o podanym rodzaju. */
async function check(page: Page, key: string, verdict: string) {
  await page.getByLabel('Klucz API').fill(key);
  await page.getByRole('button', { name: 'Sprawdź klucz' }).click();
  // Wynik kreatora (panel konta na liście może w tym czasie pokazywać własny).
  const result = verdictIn(page.getByRole('region', { name: 'Dodaj dostawcę' }));
  await expect(result).toHaveAttribute('data-verdict', verdict);
  // Fokus ląduje na tytule wyniku — przycisk, który go wywołał, już nie istnieje.
  await expect(result.getByRole('heading', { level: 4 })).toBeFocused();
}

const openAiRow = (page: Page) =>
  page
    .getByRole('list', { name: 'Twoje konta' })
    .getByRole('listitem')
    .filter({ hasText: /^OpenAI/ });

test('krok klucza: skąd wziąć klucz, podgląd i ocena wklejonego tekstu', async ({ page }) => {
  await openWizard(page);
  await expect(page.getByRole('heading', { name: 'Skąd wziąć klucz' })).toBeVisible();
  await expect(page.getByText('https://platform.openai.com/api-keys')).toBeVisible();
  await expect(page.getByText(/abonament czatu .* jej nie obejmuje/)).toBeVisible();
  const submit = page.getByRole('button', { name: 'Sprawdź klucz' });
  await expect(submit).toBeDisabled();

  const field = page.getByLabel('Klucz API');
  await expect(field).toHaveAttribute('type', 'password');
  await page.getByLabel('Pokaż klucz').check();
  await expect(field).toHaveAttribute('type', 'text');
  await page.getByLabel('Pokaż klucz').uncheck();

  // Klucz innego dostawcy, skopiowany z cudzysłowami i końcem wiersza: podpowiedź, nie blokada.
  await field.fill(`"${fakeKey('sk-ant-', 'api03')}"\n`);
  await expect(page.getByText(/To wygląda na klucz Anthropic/)).toBeVisible();
  await expect(page.getByText(/Usunę dodatki ze schowka/)).toBeVisible();
  await expect(submit).toBeEnabled();
  await field.fill('sk-krotki');
  await expect(page.getByText(/To bardzo krótki tekst/)).toBeVisible();
  // Zdanie zamiast klucza i token logowania narzędzia — tego nie wysyłamy.
  await field.fill(`Mój klucz: ${fakeKey('sk-', 'proj')}`);
  await expect(page.getByText(/To nie wygląda na klucz API/)).toBeVisible();
  await expect(submit).toBeDisabled();
  await expectReadable(page, 'kreator — to nie klucz');
  await field.fill(fakeKey('sk-', 'proj'));
  await expect(page.getByText(/Wygląda jak klucz/)).toBeVisible();
  await expect(submit).toBeEnabled();
  await expectAccessible(page, 'kreator — krok klucza');
  await expectReadable(page, 'kreator — krok klucza');
});

test('token logowania Claude Code nie jest przyjmowany jako klucz', async ({ page }) => {
  await openWizard(page, /Anthropic \(Claude\)/);
  await page.getByLabel('Klucz API').fill(fakeKey('sk-ant-oat01-', 'x'));
  await expect(page.getByText(/token logowania narzędzia Claude Code/)).toBeVisible();
  await expect(page.getByRole('button', { name: 'Sprawdź klucz' })).toBeDisabled();
});

test('zły klucz: powód, kroki naprawy i odpowiedź dostawcy; poprawiony klucz przechodzi', async ({
  page,
}) => {
  await openWizard(page);
  await check(page, fakeKey('sk-', 'bad'), 'invalid_key');
  const verdict = verdictIn(page);
  await expect(verdict).toContainText('OpenAI nie przyjął tego klucza');
  await expect(verdict).toContainText('Co zrobić');
  await expect(verdict.getByRole('listitem')).toHaveCount(4);
  await verdict.getByText('Co dokładnie odpowiedział OpenAI').click();
  await expect(verdict).toContainText('401');
  await expect(verdict).toContainText('invalid_api_key');
  await expect(verdict).toContainText('Incorrect API key provided');
  await expect(page.getByText(/Klucz odrzucony \(401\)/)).toHaveCount(0);
  // Z odrzuconym kluczem nie da się iść dalej ani „zostawić konta”.
  await expect(page.getByRole('button', { name: 'Dalej' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Zostaw konto i zamknij' })).toHaveCount(0);
  await expectAccessible(page, 'kreator — zły klucz');
  await expectReadable(page, 'kreator — zły klucz');

  await page.getByRole('button', { name: 'Popraw klucz' }).click();
  await expect(page.getByLabel('Klucz API')).toHaveValue('');
  await check(page, fakeKey('sk-', 'proj'), 'ok');
  await expect(page.getByText(/^Połączono/)).toBeVisible();
  await page.getByRole('button', { name: 'Dalej' }).click();
  await expect(page.getByText(/Wykryto \d+ model/)).toBeVisible();
  // Po poprawce zostaje jedno konto, nie dwa.
  await page.getByRole('button', { name: /Zamknij — resztę/ }).click();
  await expect(openAiRow(page)).toHaveCount(1);
});

test('klucz z ograniczonymi uprawnieniami: dalej dopiero po podaniu nazwy modelu', async ({
  page,
}) => {
  await openWizard(page);
  await check(page, fakeKey('sk-', 'restricted'), 'restricted');
  await expect(verdictIn(page)).toContainText('ma ograniczone uprawnienia');
  await expect(page.getByText(/Models: Read/)).toBeVisible();
  const next = page.getByRole('button', { name: 'Dalej' });
  await expect(next).toBeDisabled();
  await expectAccessible(page, 'kreator — klucz ograniczony');
  await expectReadable(page, 'kreator — klucz ograniczony');
  await page.getByLabel('Nazwa modelu').fill('gpt-4o-mini');
  await page.getByRole('button', { name: 'Zapisz model' }).click();
  await expect(page.getByText('Alfa będzie używać modelu „gpt-4o-mini”.')).toBeVisible();
  await expect(next).toBeEnabled();
  await next.click();
  await expect(page.getByText('Wykryto 1 model')).toBeVisible();
});

test('konto bez środków: to nie jest „zły klucz” i konto można zostawić', async ({ page }) => {
  await openWizard(page);
  await check(page, fakeKey('sk-', 'nocredit'), 'no_credit');
  await expect(verdictIn(page)).toContainText('konto nie ma środków');
  await expect(page.getByText(/Doładuj konto API/)).toBeVisible();
  await expect(page.getByText(/nowego klucza nie trzeba/)).toBeVisible();
  await expect(page.getByText(/nie przyjął tego klucza/)).toHaveCount(0);
  await expectReadable(page, 'kreator — brak środków');
  await page.getByRole('button', { name: 'Zostaw konto i zamknij' }).click();
  await expect(openAiRow(page)).toContainText('brak środków na koncie');
});

for (const [mark, verdict, title] of [
  ['region', 'region', 'nie obsługuje tego kraju lub regionu'],
  ['refused', 'refused', 'odmówił dostępu'],
  ['timeout', 'timeout', 'nie odpowiedział na czas'],
  ['ratelimit', 'rate_limited', 'chwilowo ogranicza zapytania'],
] as const) {
  test(`wynik „${verdict}”: klucz może być dobry — ponowne sprawdzenie i konto do zostawienia`, async ({
    page,
  }) => {
    await openWizard(page);
    await check(page, fakeKey('sk-', mark), verdict);
    await expect(verdictIn(page)).toContainText(title);
    await expect(page.getByRole('button', { name: 'Sprawdź ponownie' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Zostaw konto i zamknij' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Popraw klucz' })).toBeVisible();
    await expectAccessible(page, `kreator — ${verdict}`);
    await expectReadable(page, `kreator — ${verdict}`);
  });
}

test('klucz, którego nie da się sprawdzić z góry: dalej po podaniu modelu', async ({ page }) => {
  await openWizard(page);
  await check(page, fakeKey('sk-', 'unsupported'), 'unsupported');
  await expect(verdictIn(page)).toContainText('Tego klucza nie da się sprawdzić z góry');
  const next = page.getByRole('button', { name: 'Dalej' });
  await expect(next).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Sprawdź ponownie' })).toBeVisible();
  await expectAccessible(page, 'kreator — bez testu');
  await expectReadable(page, 'kreator — bez testu');
  await page.getByLabel('Nazwa modelu').fill('moj-model');
  await page.getByRole('button', { name: 'Zapisz model' }).click();
  await expect(next).toBeEnabled();
  await next.click();
  await expect(page.getByText('Wykryto 1 model')).toBeVisible();
});

test('zmiana dostawcy w kreatorze czyści klucz i adres poprzedniego', async ({ page }) => {
  await openProviders(page);
  await pickProvider(page, /Własny endpoint|OpenAI-compatible|zgodny z OpenAI/i);
  const submit = page.getByRole('button', { name: 'Sprawdź klucz' });
  await page.getByLabel('Klucz API').fill(fakeKey('sk-', 'proj'));
  // Bez adresu własnego endpointu klucza nie wysyłamy.
  await expect(submit).toBeDisabled();
  await page.getByLabel('Adres endpointu (base URL)').fill('https://moj-serwer.example/v1');
  await expect(submit).toBeEnabled();
  await page
    .getByRole('button', { name: /Dostawca/ })
    .first()
    .click();
  await page
    .getByRole('button', { name: /^OpenAI/ })
    .first()
    .click();
  await expect(page.getByLabel('Klucz API')).toHaveValue('');
  await expect(page.getByLabel('Adres endpointu (base URL)')).toHaveCount(0);
});

test('lista kont: powód przy koncie, wymiana klucza, wyłączenie i ten sam klucz drugi raz', async ({
  page,
}) => {
  await openWizard(page);
  await check(page, fakeKey('sk-', 'nocredit'), 'no_credit');
  await page.getByRole('button', { name: /Zamknij — resztę/ }).click();
  const row = openAiRow(page);
  await expect(row).toContainText('brak środków na koncie');

  await row.getByRole('button', { name: 'Co zrobić' }).click();
  await expect(row).toContainText('Doładuj konto API');
  await expectAccessible(page, 'lista kont — wyjaśnienie');
  await expectReadable(page, 'lista kont — wyjaśnienie');

  // Wymiana klucza w tym samym koncie.
  await row.getByRole('button', { name: 'Zmień klucz' }).first().click();
  await expect(row.getByLabel('Nazwa konta')).toHaveCount(0);
  await row.getByLabel('Klucz API').fill(`OPENAI_API_KEY=${fakeKey('sk-', 'proj')}`);
  await row.getByRole('button', { name: 'Sprawdź i zamień klucz' }).click();
  await expect(verdictIn(row)).toHaveAttribute('data-verdict', 'ok');
  await row.getByRole('button', { name: 'Zamknij' }).click();
  await expect(row).toContainText('działa');
  await expect(row).not.toContainText('brak środków');
  await expect(openAiRow(page)).toHaveCount(1);

  // Zły klucz nie zastępuje działającego: konto zostaje przy poprzednim.
  await row.getByRole('button', { name: 'Zmień klucz' }).click();
  await row.getByLabel('Klucz API').fill(fakeKey('sk-', 'bad'));
  await row.getByRole('button', { name: 'Sprawdź i zamień klucz' }).click();
  await expect(verdictIn(row)).toHaveAttribute('data-verdict', 'invalid_key');
  await expect(row).toContainText('konto dalej używa poprzedniego');
  await expectReadable(page, 'lista kont — nowy klucz odrzucony');
  await row.getByRole('button', { name: 'Zmień klucz' }).first().click();
  await expect(verdictIn(row)).toHaveCount(0);
  await expect(row).toContainText('działa');
  await expect(row).not.toContainText('klucz odrzucony');
  await row.getByRole('button', { name: 'Testuj połączenie' }).click();
  await expect(row).toContainText('działa');

  // Limit zapytań nie dowodzi, że nowy klucz jest dobry — działający zostaje.
  await row.getByRole('button', { name: 'Zmień klucz' }).click();
  await row.getByLabel('Klucz API').fill(fakeKey('sk-', 'ratelimit'));
  await row.getByRole('button', { name: 'Sprawdź i zamień klucz' }).click();
  await expect(verdictIn(row)).toHaveAttribute('data-verdict', 'rate_limited');
  await expect(row).toContainText('konto dalej używa poprzedniego');
  await row.getByRole('button', { name: 'Zamknij' }).click();
  await expect(row).toContainText('działa');

  // Klucz rozpoznany przez dostawcę, ale bez środków: zapisany, z możliwością ponownego testu.
  await row.getByRole('button', { name: 'Zmień klucz' }).click();
  await row.getByLabel('Klucz API').fill(fakeKey('sk-', 'nocredit'));
  await row.getByRole('button', { name: 'Sprawdź i zamień klucz' }).click();
  await expect(verdictIn(row)).toHaveAttribute('data-verdict', 'no_credit');
  await expect(row).toContainText('Nowy klucz został zapisany');
  await expect(row.getByRole('button', { name: 'Sprawdź ponownie' })).toBeVisible();
  await expectReadable(page, 'lista kont — nowy klucz bez środków');
  await row.getByRole('button', { name: 'Zamknij' }).click();
  await expect(row).toContainText('brak środków na koncie');
  // Powrót do działającego klucza: po udanej wymianie widać potwierdzenie zapisu.
  await row.getByRole('button', { name: 'Zmień klucz' }).first().click();
  await row.getByLabel('Klucz API').fill(fakeKey('sk-', 'proj2'));
  await row.getByRole('button', { name: 'Sprawdź i zamień klucz' }).click();
  await expect(verdictIn(row)).toHaveAttribute('data-verdict', 'ok');
  await expect(row).toContainText('Nowy klucz został zapisany');
  await row.getByRole('button', { name: 'Zamknij' }).click();

  // Wyłączenie i włączenie.
  await row.getByRole('button', { name: 'Wyłącz', exact: true }).click();
  await expect(row).toContainText('wyłączone');
  await expect(row.getByRole('button', { name: 'Testuj połączenie' })).toBeDisabled();
  await row.getByRole('button', { name: 'Włącz', exact: true }).click();
  await expect(row).not.toContainText('wyłączone');
  await expect(row).toContainText('działa');

  // Ten sam klucz dodany drugi raz: czytelna odmowa z nazwą istniejącego konta.
  await pickProvider(page, /^OpenAI/);
  await page.getByLabel('Klucz API').fill(fakeKey('sk-', 'proj2'));
  await page.getByRole('button', { name: 'Sprawdź klucz' }).click();
  await expect(page.getByRole('alert')).toContainText(
    'Ten klucz jest już dodany jako konto „OpenAI”',
  );
});

test('włączenie konta z odrzuconym kluczem nie robi z niego działającego', async ({ page }) => {
  await openWizard(page);
  await check(page, fakeKey('sk-', 'nocredit'), 'no_credit');
  await page.getByRole('button', { name: 'Zostaw konto i zamknij' }).click();
  const row = openAiRow(page);
  await row.getByRole('button', { name: 'Wyłącz', exact: true }).click();
  await expect(row).toContainText('wyłączone');
  await row.getByRole('button', { name: 'Włącz', exact: true }).click();
  await expect(row).toContainText('brak środków na koncie');
});

test('kreator i panel konta otwarte naraz: bez zdublowanych identyfikatorów', async ({ page }) => {
  await openWizard(page);
  await check(page, fakeKey('sk-', 'nocredit'), 'no_credit');
  await page.getByRole('button', { name: 'Zostaw konto i zamknij' }).click();
  await openAiRow(page).getByRole('button', { name: 'Co zrobić' }).click();
  await pickProvider(page, /Anthropic \(Claude\)/);
  await check(page, fakeKey('sk-ant-', 'bad'), 'invalid_key');
  await expect(verdictIn(page)).toHaveCount(2);
  const duplicates = await page.evaluate(() => {
    const ids = [...document.querySelectorAll('[id]')].map((el) => el.id);
    return ids.filter((id, at) => ids.indexOf(id) !== at);
  });
  expect(duplicates).toEqual([]);
  await expectAccessible(page, 'kreator i panel konta');
});

for (const skin of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`czytelność kreatora i wyniku: ${skin} / ${theme}`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
      await page.goto(`/?style=${skin}&theme=${theme}`);
      await waitForChat(page);
      await page.keyboard.press('Control+,');
      await page
        .getByRole('navigation', { name: 'Sekcje ustawień' })
        .getByRole('button', { name: 'Konta i klucze API' })
        .click();
      await pickProvider(page, /^OpenAI/);
      await page.getByLabel('Klucz API').fill(`"${fakeKey('sk-ant-', 'api03')}"`);
      await expectReadable(page, `${skin}/${theme} — krok klucza`);
      await check(page, fakeKey('sk-', 'restricted'), 'restricted');
      await verdictIn(page).locator('summary').click();
      await expectReadable(page, `${skin}/${theme} — klucz ograniczony`);
      await page.getByRole('button', { name: 'Popraw klucz' }).click();
      await check(page, fakeKey('sk-', 'bad'), 'invalid_key');
      await verdictIn(page).locator('summary').click();
      await expectReadable(page, `${skin}/${theme} — zły klucz`);
      await expectAccessible(page, `${skin}/${theme} — zły klucz`);
    });
  }
}

test('kreator po angielsku: wynik i kroki naprawy są przetłumaczone', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto('/');
  await waitForChat(page);
  await page.keyboard.press('Control+k');
  await expect(page.locator('dialog[open] input')).toBeFocused();
  await page.keyboard.type('angielski');
  await expect(page.locator('dialog [role=option][data-selected]')).toContainText('angielski');
  await page.keyboard.press('Enter');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await page.keyboard.press('Control+,');
  await page.getByRole('button', { name: 'API accounts and keys' }).click();
  await page.getByRole('button', { name: 'Add provider' }).click();
  await page
    .getByRole('button', { name: /^OpenAI/ })
    .first()
    .click();
  await page.getByLabel('API key').fill(fakeKey('sk-', 'bad'));
  await page.getByRole('button', { name: 'Check key' }).click();
  const verdict = verdictIn(page);
  await expect(verdict).toContainText('OpenAI did not accept this key');
  await expect(verdict.getByRole('listitem')).toHaveCount(4);
  await expect(verdict).not.toContainText(/klucz|dostawc/i);
  await expect(verdict).toContainText('Incorrect API key provided: [•••]');
  await expect(page.getByRole('button', { name: 'Fix the key' })).toBeVisible();
});
