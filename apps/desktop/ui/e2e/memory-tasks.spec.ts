import { expect, test, type Page } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

async function settings(page: Page, section: string) {
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  if (!(await nav.isVisible())) await page.keyboard.press('Control+,');
  await nav.getByRole('button', { name: section }).click();
  await expect(page.getByRole('heading', { name: section, level: 2 })).toBeVisible();
}

for (const theme of ['light', 'dark'] as const) {
  test.describe(`F5–F7 — motyw ${theme}`, () => {
    test('Inspektor pamięci: filtry, „dlaczego”, podgląd kaskady zapomnienia', async ({ page }) => {
      await page.setViewportSize({ width: 1600, height: 900 });
      await openApp(page, { theme });
      await waitForChat(page);
      await page.keyboard.press('Alt+4');
      await expect(page.getByRole('tab', { name: 'Pamięć', selected: true })).toBeVisible();
      const list = page.getByRole('list', { name: 'Wpisy pamięci' });
      await expect(list.getByRole('button').first()).toBeVisible();
      await expectAccessible(page, `inspektor ${theme}`);
      await list.getByRole('button', { name: /do piątku/ }).click();
      const details = page.getByRole('region', { name: 'Szczegóły wpisu' });
      await expect(details.getByRole('heading', { name: 'Dlaczego to pamiętam' })).toBeVisible();
      await expect(details.getByRole('heading', { name: 'Historia wersji' })).toBeVisible();
      await details.getByRole('button', { name: 'Zapomnij', exact: true }).click();
      await expect(page.getByRole('group', { name: 'Co zniknie' })).toBeVisible();
      await expectAccessible(page, `inspektor szczegóły ${theme}`);
      await page.getByRole('button', { name: 'Zapomnij na zawsze' }).click();
      await expect(list.getByRole('button', { name: /do piątku/ })).toHaveCount(0);
    });

    test('panel Zadania: drzewo DAG, nowe zadanie, anulowanie', async ({ page }) => {
      await page.setViewportSize({ width: 1600, height: 900 });
      await openApp(page, { theme });
      await waitForChat(page);
      await page.keyboard.press('Alt+7');
      await expect(page.getByRole('tab', { name: 'Zadania', selected: true })).toBeVisible();
      const tree = page.getByRole('list', { name: 'Drzewo zadań' });
      await expect(tree).toContainText('Policz marże i trendy');
      await expect(tree).toContainText('Most CLI — wynik niezweryfikowany przez Alfę');
      await expectAccessible(page, `zadania ${theme}`);
      await page.getByLabel('Cel zadania').fill('Sprawdź pisownię raportu');
      await page.getByRole('button', { name: 'Dodaj zadanie' }).click();
      await expect(tree).toContainText('Sprawdź pisownię raportu');
      const running = page.getByRole('group', { name: 'Akcje zadania „Policz marże i trendy"' });
      await running.getByRole('button', { name: 'Anuluj' }).click();
      await expect(
        tree.locator('[data-state="done"][data-result="cancelled"]').first(),
      ).toBeVisible();
    });

    test('Wyzwalacze: podgląd cron, utworzenie, uruchom teraz, dziennik', async ({ page }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await settings(page, 'Zadania w tle i wyzwalacze');
      await expect(page.getByText(/Obserwacja katalogów jest niedostępna/).first()).toBeVisible();
      await expect(page.getByRole('list', { name: 'Najbliższe uruchomienia' })).toBeVisible();
      await expectAccessible(page, `wyzwalacze ${theme}`);
      await page.getByLabel('Nazwa', { exact: true }).fill('Kopia notatek');
      await page.getByLabel('Co ma zrobić agentka').fill('Zrób kopię notatek');
      await page.getByRole('button', { name: 'Utwórz wyzwalacz' }).click();
      const list = page.getByRole('list', { name: 'Lista wyzwalaczy' });
      await expect(list).toContainText('Kopia notatek');
      await page.getByRole('button', { name: 'Uruchom teraz' }).first().click();
      await expect(page.getByRole('heading', { name: 'Dziennik uruchomień' })).toBeVisible();
      await expect(page.getByText(/zadanie zgłoszone/).first()).toBeVisible();
    });

    test('Reguły Marszałka: propozycja → podgląd zawężenia → zatwierdzenie → cofnięcie', async ({
      page,
    }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await settings(page, 'Reguły Marszałka');
      await page.getByLabel('Polecenie dla Marszałka').fill('Nie używaj mostów CLI');
      await page.getByRole('button', { name: 'Zaproponuj reguły' }).click();
      await expect(page.getByRole('heading', { name: 'Podgląd zawężenia' })).toBeVisible();
      await expectAccessible(page, `marszałek ${theme}`);
      await page.getByRole('button', { name: 'Zatwierdź' }).click();
      await expect(page.getByText('mosty CLI zabronione').first()).toBeVisible();
      await page.getByRole('button', { name: 'Cofnij regułę: Bez mostów CLI' }).click();
      await expect(page.getByText('mosty CLI dozwolone')).toBeVisible();
    });

    test('karty zgodności mostów i Ustawienia → Pamięć', async ({ page }) => {
      await openApp(page, { theme });
      await waitForChat(page);
      await settings(page, 'Modele i dostawcy');
      const cards = page.getByRole('list', { name: 'Karty zgodności mostów' });
      await expect(cards).toContainText('Claude Code (CLI)');
      await expect(cards).toContainText('Zabroniona');
      await expectAccessible(page, `mosty ${theme}`);
      const claude = cards.getByRole('listitem').filter({ hasText: 'Claude Code (CLI)' });
      await claude.getByRole('button', { name: 'Zaloguj w terminalu' }).click();
      await expect(claude.getByText('claude /login')).toBeVisible();
      await claude.getByRole('button', { name: 'Przypnij wykrytą wersję' }).click();
      await expect(claude.getByText('Wersja zgodna z przypięciem.')).toBeVisible();
      await settings(page, 'Pamięć');
      await expect(page.getByRole('heading', { name: 'Porządkowanie pamięci' })).toBeVisible();
      await expectAccessible(page, `pamięć ustawienia ${theme}`);
      await page.getByRole('button', { name: 'Porządkuj teraz' }).click();
      await expect(page.getByText(/Ostatnio/).first()).toBeVisible();
    });
  });
}
