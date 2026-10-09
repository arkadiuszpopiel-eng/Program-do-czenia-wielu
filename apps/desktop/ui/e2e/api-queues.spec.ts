import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Ustawienia → API i kolejki modeli (zgłoszenie właściciela 2026-10-09: konto ze 133 modelami,
// rozmowa szła do modelu syntezy mowy, a wybrać nie dało się żadnego). Kolejka do dziesięciu
// modeli na klasę zadań, przeglądarka pełnej listy z filtrami, dobieranie automatyczne.

async function openApi(page: Page, query = '') {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto(`/${query}`);
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'API i kolejki modeli' })
    .click();
  await page.getByRole('tab', { name: /Praca — OpenAI/ }).click();
}

const queue = (page: Page) => page.locator('ol.queue > li');
const ids = (page: Page) => queue(page).locator('code').allTextContents();

test('kolejka rozmowy: kolejność, usuwanie i dodawanie z listy modeli', async ({ page }) => {
  await openApi(page);
  await expect(
    page.getByText('17 modeli do rozmowy · wybór automatyczny: gpt-6-sol'),
  ).toBeVisible();
  expect(await ids(page)).toEqual(['gpt-6-sol', 'gpt-6-luna', 'gpt-5.5']);
  await expect(queue(page).first()).toContainText('pierwszy wybór');
  await expect(queue(page).nth(1)).toContainText('zapas 1');

  // Kolejność: pierwszy niżej, ostatni wyżej; skrajne przyciski są wyłączone.
  await expect(page.getByRole('button', { name: 'Wyżej: gpt-6-sol' })).toBeDisabled();
  await page.getByRole('button', { name: 'Niżej: gpt-6-sol' }).click();
  await expect.poll(() => ids(page)).toEqual(['gpt-6-luna', 'gpt-6-sol', 'gpt-5.5']);
  await page.getByRole('button', { name: 'Wyżej: gpt-5.5' }).click();
  await expect.poll(() => ids(page)).toEqual(['gpt-6-luna', 'gpt-5.5', 'gpt-6-sol']);
  await expect(page.getByRole('button', { name: 'Niżej: gpt-6-sol' })).toBeDisabled();
  await page.getByRole('button', { name: 'Usuń z kolejki: gpt-5.5' }).click();
  await expect.poll(() => ids(page)).toEqual(['gpt-6-luna', 'gpt-6-sol']);

  // Dodanie z przeglądarki: wyszukiwanie, model trafia na koniec, w liście jest już „w kolejce”.
  await page.getByLabel('Szukaj modelu').fill('5 mini');
  await page.getByRole('button', { name: 'Dodaj do kolejki: gpt-5-mini' }).click();
  await expect.poll(() => ids(page)).toEqual(['gpt-6-luna', 'gpt-6-sol', 'gpt-5-mini']);
  const row = page.locator('ul.models > li').filter({ hasText: 'gpt-5-mini' }).first();
  await expect(row).toContainText('w kolejce');
  await expect(page.getByRole('tab', { name: /^Rozmowa/ })).toContainText('3');

  // Zmiana zostaje po ponownym wejściu na stronę.
  await page.getByRole('button', { name: 'Modele i dostawcy', exact: true }).click();
  await page.getByRole('button', { name: 'API i kolejki modeli' }).click();
  await page.getByRole('tab', { name: /Praca — OpenAI/ }).click();
  expect(await ids(page)).toEqual(['gpt-6-luna', 'gpt-6-sol', 'gpt-5-mini']);
});

test('model syntezy mowy nie wejdzie do rozmowy — ani z listy, ani wpisany ręcznie', async ({
  page,
}) => {
  await openApi(page);
  // Domyślnie lista pokazuje tylko modele pasujące do klasy; filtr rodzaju odsłania resztę.
  await expect(page.locator('ul.models')).not.toContainText('tts-1');
  await page.getByRole('button', { name: /^Synteza mowy/ }).click();
  const speech = page.locator('ul.models > li');
  await expect(speech).toHaveCount(3);
  for (const row of await speech.all()) await expect(row).toContainText('nie do tej klasy');
  await expect(page.getByRole('button', { name: 'Dodaj do kolejki: tts-1' })).toHaveCount(0);

  // Ręcznie wpisany model też przechodzi przez reguły rdzenia: odmowa z komunikatem.
  await page.getByLabel('Model spoza listy').fill('tts-1');
  await page.getByRole('button', { name: 'Dodaj do kolejki', exact: true }).click();
  await expect(page.getByText(/nie nadaje się do tej klasy zadań/)).toBeVisible();
  expect(await ids(page)).toEqual(['gpt-6-sol', 'gpt-6-luna', 'gpt-5.5']);
  await page.getByLabel('Model spoza listy').fill('gpt-moj-finetune');
  await page.getByRole('button', { name: 'Dodaj do kolejki', exact: true }).click();
  await expect(queue(page).last()).toContainText('gpt-moj-finetune');
  await expect(queue(page).last()).toContainText('nie ma go na liście konta');
});

test('dobieranie automatyczne, pełna kolejka i powrót do wyboru automatycznego', async ({
  page,
}) => {
  await openApi(page);
  await page.getByRole('button', { name: 'Dobierz automatycznie' }).click();
  await page.getByRole('menuitem', { name: 'Oszczędnie i szybko' }).click();
  await expect(page.getByText('Kolejka „Rozmowa” ułożona: Oszczędnie i szybko.')).toBeVisible();
  await expect(queue(page)).toHaveCount(10);
  await expect(queue(page).first()).toContainText('nano');
  await expect(page.getByText('10 z 10')).toBeVisible();
  // Pełna kolejka: lista modeli mówi dlaczego nie da się dodać kolejnego.
  await expect(page.locator('ul.models')).toContainText('kolejka pełna');
  await expectAccessible(page, 'API — pełna kolejka');
  await expectReadable(page, 'API — pełna kolejka');

  await page.getByRole('button', { name: 'Wyczyść kolejkę' }).click();
  await expect(queue(page)).toHaveCount(0);
  await expect(page.getByText('Kolejka jest pusta — Alfa użyje modelu „gpt-6-sol”.')).toBeVisible();
  await expect(page.getByRole('tab', { name: /^Rozmowa/ })).toContainText('auto');
});

test('każda klasa zadań ma własną kolejkę; embeddingi przyjmują tylko embeddingi', async ({
  page,
}) => {
  await openApi(page);
  const tab = page.getByRole('tab', { name: /^Rozmowa/ });
  await tab.focus();
  // Strzałki przełączają klasy jak zakładki.
  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('tab', { name: /^Kod/ })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('tab', { name: /^Kod/ })).toBeFocused();
  await expect(page.getByText('Kolejka: Kod')).toBeVisible();
  await expect(queue(page)).toHaveCount(0);
  await page.keyboard.press('ArrowLeft');
  await expect(queue(page)).toHaveCount(3);

  await page.getByRole('tab', { name: /^Embeddingi/ }).click();
  await expect(page.getByText(/nie ma jeszcze modeli wybranych do tej klasy/)).toBeVisible();
  const models = page.locator('ul.models > li');
  await expect(models).toHaveCount(2);
  await page.getByRole('button', { name: 'Dodaj do kolejki: text-embedding-3-large' }).click();
  expect(await ids(page)).toEqual(['text-embedding-3-large']);
  await expect(page.getByRole('tab', { name: /^Embeddingi/ })).toContainText('1');
  // Kolejka rozmowy zostaje nietknięta.
  await page.getByRole('tab', { name: /^Rozmowa/ }).click();
  expect(await ids(page)).toEqual(['gpt-6-sol', 'gpt-6-luna', 'gpt-5.5']);

  // Planowanie dopuszcza modele rozumujące, szybka odpowiedź głosowa — nie.
  await page.getByRole('tab', { name: /^Planowanie/ }).click();
  await page.getByLabel('Szukaj modelu').fill('o3');
  await expect(page.getByRole('button', { name: 'Dodaj do kolejki: o3' })).toBeVisible();
  await page.getByRole('tab', { name: /^Głos/ }).click();
  await expect(page.getByText('Żaden model nie pasuje do filtrów.')).toBeVisible();
});

test('drugie konto ma własne kolejki i własną listę modeli', async ({ page }) => {
  await openApi(page);
  await page.getByRole('tab', { name: /Anthropic — prywatne/ }).click();
  await expect(queue(page)).toHaveCount(0);
  await expect(page.locator('ul.models > li')).toHaveCount(2);
  await page.getByRole('button', { name: 'Dodaj do kolejki: claude-haiku-5' }).click();
  expect(await ids(page)).toEqual(['claude-haiku-5']);
  await page.getByRole('tab', { name: /Praca — OpenAI/ }).click();
  expect(await ids(page)).toEqual(['gpt-6-sol', 'gpt-6-luna', 'gpt-5.5']);
});

test('bez kont: strona prowadzi do dodania dostawcy', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto('/?scenario=no-keys');
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page.getByRole('button', { name: 'API i kolejki modeli' }).click();
  await expect(page.getByText('Nie masz jeszcze konta API')).toBeVisible();
  await page.getByRole('button', { name: 'Dodaj dostawcę' }).click();
  await expect(page.getByRole('heading', { name: 'Modele i dostawcy' })).toBeVisible();
});

for (const skin of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`czytelność strony API i otwartego menu: ${skin} / ${theme}`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
      await page.goto(`/?style=${skin}&theme=${theme}`);
      await waitForChat(page);
      await page.keyboard.press('Control+,');
      await page.getByRole('button', { name: 'API i kolejki modeli' }).click();
      await page.getByRole('tab', { name: /Praca — OpenAI/ }).click();
      await expectReadable(page, `${skin}/${theme} — kolejka`);
      await expectAccessible(page, `${skin}/${theme} — kolejka`);
      await page.getByRole('button', { name: 'Dobierz automatycznie' }).click();
      await expect(page.getByRole('menuitem', { name: 'Najlepsza jakość' })).toBeVisible();
      await expectReadable(page, `${skin}/${theme} — menu dobierania`);
      await page.keyboard.press('Escape');
      await page.getByRole('button', { name: /^Synteza mowy/ }).click();
      await page.getByRole('tab', { name: /^Embeddingi/ }).click();
      await expectReadable(page, `${skin}/${theme} — pusta kolejka i filtr`);
    });
  }
}

test('strona API po angielsku', async ({ page }) => {
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
  await page.getByRole('button', { name: 'API and model queues' }).click();
  await page.getByRole('tab', { name: /Praca — OpenAI/ }).click();
  await expect(page.getByRole('heading', { name: 'Queue: Chat' })).toBeVisible();
  await expect(queue(page).first()).toContainText('first choice');
  await expect(page.locator('main')).not.toContainText(/Kolejka|Rozmowa|zapas/);
});

test('test mikrofonu pokazuje poziom po redukcji szumów i surowy sygnał', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto('/');
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page.getByRole('button', { name: 'Głos', exact: true }).click();
  await expect(page.getByText(/Niebieski pasek to sygnał po redukcji szumów/)).toBeVisible();
  await page.getByRole('button', { name: 'Test mikrofonu' }).click();
  await expect(
    page.getByText(/Po redukcji szumów: \d+% · surowy sygnał z mikrofonu: \d+%/),
  ).toBeVisible();
  // Ustawienie siły redukcji jest na tej samej stronie i ma trzy poziomy.
  await expect(page.getByText('Redukcja szumów mikrofonu').first()).toBeVisible();
  await expectReadable(page, 'głos — test mikrofonu');
});

test('z listy kont jednym kliknięciem do kolejek tego konta i z powrotem', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto('/');
  await waitForChat(page);
  await page.keyboard.press('Control+,');
  await page.getByRole('button', { name: 'Modele i dostawcy' }).click();
  const row = page
    .getByRole('list', { name: 'Twoje konta' })
    .getByRole('listitem')
    .filter({ hasText: 'Praca — OpenAI' });
  await row.getByRole('button', { name: 'Modele i kolejki' }).click();
  await expect(page.getByRole('heading', { name: 'API i kolejki modeli' })).toBeVisible();
  await expect(page.getByRole('tab', { name: /Praca — OpenAI/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  expect(await ids(page)).toEqual(['gpt-6-sol', 'gpt-6-luna', 'gpt-5.5']);
  await page.getByRole('button', { name: /Klucz, test i limit tego konta/ }).click();
  await expect(page.getByRole('heading', { name: 'Modele i dostawcy' })).toBeVisible();
});

test('szczegóły pod odpowiedzią (model, tokeny, koszt, czas) można wyłączyć', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await page.goto('/');
  await waitForChat(page);
  const details = page.locator('p.usage');
  await expect(details.first()).toBeVisible();
  await expect(details.first()).toContainText('tok.');
  await page.keyboard.press('Control+,');
  await page.getByRole('button', { name: 'Wygląd' }).click();
  await page.getByRole('switch', { name: 'Szczegóły pod odpowiedzią' }).click();
  await page.getByRole('button', { name: 'Wróć do rozmowy' }).click();
  await waitForChat(page);
  await expect(details).toHaveCount(0);
});
