import { expect, test, type Locator, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Ustawienia dla firm istniejącego konta (C2.12, wersja 0.0.65): organizacja i projekt OpenAI oraz
// wersja API i wdrożenia Azure OpenAI zmieniane na karcie konta — bez usuwania konta i ponownego
// wpisywania klucza. Każdy stan formularza w każdym stylu i motywie.

const SKINS = ['classic', 'glass', 'next'] as const;
const THEMES = ['light', 'dark'] as const;
const fakeKey = (prefix: string) => `${prefix}${'XXXX'.repeat(8)}`;

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

const row = (page: Page, label: string) =>
  page.getByRole('list', { name: 'Twoje konta' }).getByRole('listitem').filter({ hasText: label });
const form = (scope: Locator) => scope.getByRole('region', { name: /^Ustawienia dla firm/ });

async function openForm(page: Page, label: string) {
  const account = row(page, label);
  await account.getByRole('button', { name: 'Ustawienia dla firm' }).click();
  await expect(form(account)).toBeVisible();
  return account;
}

for (const skin of SKINS) {
  for (const theme of THEMES) {
    test(`ustawienia dla firm OpenAI: błąd, zapis i skrót — ${skin}/${theme}`, async ({ page }) => {
      await openProviders(page, `?style=${skin}&theme=${theme}`);
      const account = await openForm(page, 'Praca — OpenAI');
      const panel = form(account);
      const save = panel.getByRole('button', { name: 'Zapisz ustawienia' });
      await expectAccessible(page, `firmy — formularz ${skin}/${theme}`);
      await expectReadable(page, 'section.content');

      // Zły identyfikator: powód pod polami, zapis zablokowany.
      await panel.getByLabel('Identyfikator organizacji').fill('acme');
      await expect(panel.getByRole('alert')).toContainText('zaczyna się od „org-”');
      await expect(save).toBeDisabled();
      await expectAccessible(page, `firmy — błąd ${skin}/${theme}`);
      await expectReadable(page, 'section.content');

      await panel.getByLabel('Identyfikator organizacji').fill(' org-acmecorporation1234 ');
      await panel.getByLabel('Identyfikator projektu').fill('proj_budgetmarketing9876');
      await expect(panel.getByRole('alert')).toHaveCount(0);
      await save.click();
      await expect(panel.getByRole('status')).toContainText('Zapisano');
      await expect(panel.getByLabel('Identyfikator organizacji')).toHaveValue(
        'org-acmecorporation1234',
      );
      await expect(account).toContainText('Konto firmowe: org-…1234 · proj_…9876');
      await expectAccessible(page, `firmy — zapisano ${skin}/${theme}`);
      await expectReadable(page, 'section.content');

      await panel.getByRole('button', { name: 'Zamknij' }).click();
      await expect(form(account)).toHaveCount(0);
    });
  }
}

test('ustawienia dla firm są tylko przy kontach OpenAI i Azure OpenAI', async ({ page }) => {
  await openProviders(page);
  await expect(
    row(page, 'Anthropic — prywatne').getByRole('button', { name: 'Ustawienia dla firm' }),
  ).toHaveCount(0);
  const account = await openForm(page, 'Praca — OpenAI');
  // Wyczyszczenie pól zapisuje zwykłe konto (bez skrótu „Konto firmowe”).
  await form(account).getByLabel('Identyfikator organizacji').fill('org-acme1234');
  await form(account).getByRole('button', { name: 'Zapisz ustawienia' }).click();
  await expect(account).toContainText('Konto firmowe');
  await form(account).getByLabel('Identyfikator organizacji').fill('');
  await form(account).getByRole('button', { name: 'Zapisz ustawienia' }).click();
  await expect(form(account).getByRole('status')).toContainText('Zapisano');
  await expect(account).not.toContainText('Konto firmowe');
});

/** Kreator: dodaje konto z kluczem (opcjonalnie projekt OpenAI) i zamyka kreator po sprawdzeniu. */
async function addAccount(
  page: Page,
  provider: RegExp,
  fill: () => Promise<void>,
  key: string,
  label?: string,
) {
  await page.getByRole('button', { name: 'Dodaj dostawcę' }).click();
  await page.getByRole('button', { name: provider }).first().click();
  await page.getByLabel('Klucz API').fill(key);
  if (label) await page.getByLabel('Nazwa konta').fill(label);
  await fill();
  await page.getByRole('button', { name: 'Sprawdź klucz' }).click();
  await expect(page.locator('[data-verdict]').first()).toHaveAttribute('data-verdict', 'ok');
  await page.getByRole('button', { name: 'Dalej' }).click();
  await page.getByRole('button', { name: /Zamknij — resztę/ }).click();
}

for (const skin of SKINS) {
  for (const theme of THEMES) {
    test(`Azure OpenAI: adres zasobu stały, wdrożenia i wersja API — ${skin}/${theme}`, async ({
      page,
    }) => {
      await openProviders(page, `?style=${skin}&theme=${theme}`);
      await addAccount(
        page,
        /^Azure OpenAI/,
        async () => {
          await page.getByLabel('Adres zasobu Azure OpenAI').fill('https://firma.openai.azure.com');
          await page.getByLabel('Nazwa wdrożenia').fill('prod-4o');
          await page.getByLabel('Model (np. gpt-4o)').fill('gpt-4o');
        },
        fakeKey('azure-'),
      );

      const account = await openForm(page, 'Azure OpenAI');
      const panel = form(account);
      await expect(panel).toContainText('Adres zasobu: https://firma.openai.azure.com.');
      await expect(panel.getByLabel('Identyfikator organizacji')).toHaveCount(0);
      await expectAccessible(page, `firmy — Azure ${skin}/${theme}`);
      await expectReadable(page, 'section.content');

      // Bez wdrożenia konto Azure nie działa — zapis zablokowany z powodem.
      await panel.getByRole('button', { name: 'Usuń wdrożenie prod-4o' }).click();
      await expect(panel.getByRole('alert')).toContainText('co najmniej jedno wdrożenie');
      await expect(panel.getByRole('button', { name: 'Zapisz ustawienia' })).toBeDisabled();
      await expectAccessible(page, `firmy — Azure bez wdrożeń ${skin}/${theme}`);
      await expectReadable(page, 'section.content');

      await panel.getByRole('button', { name: 'Dodaj wdrożenie' }).click();
      await panel.getByLabel('Nazwa wdrożenia').fill('prod-mini');
      await panel.getByLabel('Model (np. gpt-4o)').fill('gpt-4o-mini');
      await panel.getByLabel('Wersja API').fill('2025-04-01-preview');
      await panel.getByRole('button', { name: 'Zapisz ustawienia' }).click();
      await expect(panel.getByRole('status')).toContainText('Zapisano');
      await expect(account).toContainText('Konto firmowe: gpt-4o-mini → prod-mini');
      await expectAccessible(page, `firmy — Azure zapisano ${skin}/${theme}`);
      await expectReadable(page, 'section.content');
    });

    test(`odmowa huba: ten sam klucz w tym samym projekcie OpenAI — ${skin}/${theme}`, async ({
      page,
    }) => {
      await openProviders(page, `?style=${skin}&theme=${theme}`);
      const key = fakeKey('sk-');
      const project = 'proj_teambmarketing5678';
      await addAccount(page, /^OpenAI/, async () => {}, key, 'Zespół A');
      // Ten sam klucz, inny projekt — to osobne konto (osobne rozliczenie), hub je przyjmuje.
      await addAccount(
        page,
        /^OpenAI/,
        async () => {
          await page
            .getByRole('checkbox', { name: 'Klucz należy do organizacji albo projektu w OpenAI' })
            .check();
          await page.getByLabel('Identyfikator projektu').fill(project);
        },
        key,
        'Zespół B',
      );

      // Przeniesienie „Zespół A” do projektu „Zespół B” zrobiłoby z nich jedno konto — odmowa.
      const account = await openForm(page, 'Zespół A');
      const panel = form(account);
      await panel.getByLabel('Identyfikator projektu').fill(project);
      await panel.getByRole('button', { name: 'Zapisz ustawienia' }).click();
      await expect(panel.getByRole('alert')).toContainText(
        'Ten klucz w tym projekcie OpenAI jest już kontem „Zespół B” — wybierz inny projekt.',
      );
      await expect(account).not.toContainText('Konto firmowe');
      await expectAccessible(page, `firmy — odmowa huba ${skin}/${theme}`);
      await expectReadable(page, 'section.content');
    });
  }
}
