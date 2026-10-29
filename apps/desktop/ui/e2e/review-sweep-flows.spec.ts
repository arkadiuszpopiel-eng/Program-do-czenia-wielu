import { expect, test, type Page } from '@playwright/test';
import { waitForChat } from './helpers';
import { STYLES, THEMES, layoutDefects, watchConsole } from './layout';

// Druga część przeglądu układu (zgłoszenie właściciela 2026-10-11): to, czego nie widać po samym
// wejściu na stronę — każdy krok pierwszego uruchomienia, kreator dostawcy, okno potwierdzenia,
// każda lista wyboru na każdej stronie Ustawień w stanie otwartym, stany systemowe (brak sieci,
// brak kluczy, brak mikrofonu…) i okna poboczne. Każde w trzech stylach i dwóch motywach.

test.describe.configure({ timeout: 300_000 });

async function open(page: Page, style: string, theme: 'light' | 'dark', extra = '', path = '/') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`${path}?style=${style}${extra}`);
}

async function openSettings(page: Page, section: string) {
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: section, exact: true })
    .click();
  await expect(page.getByRole('heading', { name: section, level: 2 })).toBeVisible();
}

for (const style of STYLES) {
  for (const theme of THEMES) {
    test(`pierwsze uruchomienie krok po kroku — ${style} ${theme}`, async ({ page }) => {
      const errors = watchConsole(page);
      await open(page, style, theme, '&scenario=first-run');
      // Start atrapy pod obciążeniem bywa dłuższy niż domyślne 5 s (jak `waitForChat`).
      await expect(page.getByRole('heading', { name: 'Witaj w Alfie' })).toBeVisible({
        timeout: 15_000,
      });
      const found: string[] = [];
      for (let step = 0; step < 8; step += 1) {
        await page.waitForTimeout(150);
        for (const defect of await layoutDefects(page)) found.push(`krok ${step + 1}: ${defect}`);
        if (step === 3) await page.getByRole('button', { name: 'Pomiń — dodam później' }).click();
        else await page.getByRole('button', { name: step === 7 ? 'Zaczynamy' : 'Dalej' }).click();
      }
      await waitForChat(page);
      expect(found, 'wady układu pierwszego uruchomienia').toEqual([]);
      expect(errors, 'błędy w konsoli').toEqual([]);
    });

    test(`kreator dostawcy i okno potwierdzenia — ${style} ${theme}`, async ({ page }) => {
      const errors = watchConsole(page);
      await open(page, style, theme);
      await waitForChat(page);
      await openSettings(page, 'Konta i klucze API');
      const found: string[] = [];
      const check = async (label: string) => {
        await page.waitForTimeout(200);
        for (const defect of await layoutDefects(page)) found.push(`${label}: ${defect}`);
      };
      // Okno potwierdzenia: usunięcie konta. Escape rezygnuje i konto zostaje.
      const account = page.getByRole('listitem').filter({ hasText: 'Praca — OpenAI' });
      await account.getByRole('button', { name: 'Usuń konto' }).click();
      const confirm = page.getByRole('alertdialog').or(page.getByRole('dialog')).last();
      await expect(confirm).toBeVisible();
      await check('potwierdzenie usunięcia konta');
      await page.keyboard.press('Escape');
      await expect(confirm).toHaveCount(0);
      await expect(account).toBeVisible();
      // Kreator: lista dostawców, potem krok klucza.
      await page.getByRole('button', { name: 'Dodaj dostawcę' }).click();
      await expect(page.getByRole('heading', { name: 'Dodaj dostawcę' })).toBeVisible();
      await check('kreator — lista dostawców');
      await page
        .getByRole('button', { name: /^OpenAI/ })
        .first()
        .click();
      await expect(page.getByLabel('Klucz API')).toBeFocused();
      await check('kreator — krok klucza');
      expect(found, 'wady układu kreatora i potwierdzenia').toEqual([]);
      expect(errors, 'błędy w konsoli').toEqual([]);
    });

    test(`każda lista wyboru w Ustawieniach otwarta — ${style} ${theme}`, async ({ page }) => {
      const errors = watchConsole(page);
      await open(page, style, theme);
      await waitForChat(page);
      await page.keyboard.press('Control+,');
      const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
      await expect(nav.locator('button.node').first()).toBeVisible();
      const pages = await nav.locator('button.node').count();
      const found: string[] = [];
      let opened = 0;
      for (let i = 0; i < pages; i += 1) {
        const node = nav.locator('button.node').nth(i);
        const name = ((await node.innerText()).split('\n')[0] ?? '').trim();
        await node.click();
        await expect(page.getByRole('heading', { level: 2, name })).toBeVisible();
        const selects = page.getByRole('main').getByRole('combobox');
        const count = await selects.count();
        for (let s = 0; s < count; s += 1) {
          const select = selects.nth(s);
          if (!(await select.isVisible()) || (await select.isDisabled())) continue;
          await select.scrollIntoViewIfNeeded();
          await select.click();
          const listId = await select.getAttribute('aria-controls');
          if (!listId) {
            // Pole z podpowiedziami, nie lista wyboru — nie otwiera listy po kliknięciu.
            await page.keyboard.press('Escape');
            continue;
          }
          const list = page.locator(`[id="${listId}"]`);
          // Pod obciążeniem pierwsze kliknięcie potrafi trafić przed podpięciem obsługi listy.
          if (!(await list.isVisible())) {
            await page.waitForTimeout(300);
            if (!(await list.isVisible())) await select.click();
          }
          await expect(list, `${name}: lista ${s + 1} się otwiera`).toBeVisible();
          opened += 1;
          const box = await list.boundingBox();
          const view = page.viewportSize();
          if (box && view) {
            const inside =
              box.x >= -1 &&
              box.y >= -1 &&
              box.x + box.width <= view.width + 1 &&
              box.y + box.height <= view.height + 1;
            if (!inside)
              found.push(
                `${name}: lista ${s + 1} wychodzi poza okno (${Math.round(box.x)},${Math.round(box.y)} ${Math.round(box.width)}×${Math.round(box.height)})`,
              );
          }
          expect(
            await list.getByRole('option').count(),
            `${name}: lista ${s + 1} ma opcje`,
          ).toBeGreaterThan(0);
          await page.keyboard.press('Escape');
          await expect(list, `${name}: Escape zamyka listę ${s + 1}`).toBeHidden();
        }
      }
      expect(opened, 'liczba otwartych list wyboru').toBeGreaterThan(10);
      expect(found, 'wady otwartych list wyboru').toEqual([]);
      expect(errors, 'błędy w konsoli').toEqual([]);
    });

    test(`aktualizacja: postęp, „Co nowego” i potwierdzenie przywrócenia — ${style} ${theme}`, async ({
      page,
    }) => {
      const errors = watchConsole(page);
      await open(page, style, theme);
      await waitForChat(page);
      await openSettings(page, 'Aktualizacje');
      const found: string[] = [];
      const check = async (label: string) => {
        await page.waitForTimeout(200);
        for (const defect of await layoutDefects(page)) found.push(`${label}: ${defect}`);
      };
      await page.getByRole('button', { name: 'Sprawdź teraz' }).click();
      await expect(page.getByText('Dostępna jest wersja 0.2.0.')).toBeVisible();
      await check('dostępna wersja');
      await page.getByRole('button', { name: 'Pobierz i przygotuj' }).click();
      await expect(
        page.getByRole('progressbar', { name: 'Pobieranie aktualizacji' }),
      ).toBeVisible();
      await check('pobieranie');
      const restart = page.getByRole('button', { name: 'Uruchom ponownie, aby zaktualizować' });
      await expect(restart).toBeVisible({ timeout: 10_000 });
      await check('gotowa do ponownego uruchomienia');
      await restart.click();
      const news = page.getByRole('dialog', { name: 'Co nowego w wersji 0.2.0' });
      await expect(news).toBeVisible();
      await check('okno „Co nowego”');
      await expect(news.getByRole('button', { name: 'Historia zmian' })).toBeVisible();
      await page.keyboard.press('Escape');
      await expect(news).toBeHidden();
      await page.getByRole('button', { name: 'Przywróć poprzednią wersję (0.1.0-f1)' }).click();
      const confirm = page.getByRole('alertdialog', { name: 'Przywrócić poprzednią wersję?' });
      await expect(confirm).toBeVisible();
      await check('potwierdzenie przywrócenia');
      await page.keyboard.press('Escape');
      await expect(confirm).toBeHidden();
      expect(found, 'wady układu aktualizacji').toEqual([]);
      expect(errors, 'błędy w konsoli').toEqual([]);
    });

    test(`menu wiadomości, Kreator agentek i strony z formularzami — ${style} ${theme}`, async ({
      page,
    }) => {
      const errors = watchConsole(page);
      await open(page, style, theme);
      await waitForChat(page);
      const found: string[] = [];
      const check = async (label: string) => {
        await page.waitForTimeout(200);
        for (const defect of await layoutDefects(page)) found.push(`${label}: ${defect}`);
      };
      // Prawy przycisk na wiadomości przypina jej pasek akcji (kopiuj, ponów…); drugi — odpina.
      // Ostatnia wiadomość — lista jest wirtualna, pierwsza (poza oknem) bywa odmontowana.
      const message = page.locator('article.msg').last();
      await message.click({ button: 'right', position: { x: 12, y: 12 } });
      await expect(message).toHaveClass(/pinned/);
      await check('wiadomość z przypiętym paskiem akcji');
      await message.click({ button: 'right', position: { x: 12, y: 12 } });
      await expect(message).not.toHaveClass(/pinned/);
      // Kreator agentek: propozycja szkicu z opisu i podgląd — pełny przebieg formularza.
      await openSettings(page, 'Kreator agentek');
      await page
        .getByRole('textbox')
        .first()
        .fill('Agentka Zofia, która porządkuje pobrane pliki.');
      await check('kreator agentek — opis');
      for (const name of [
        'Zadania w tle i wyzwalacze',
        'Reguły Marszałka',
        'Umiejętności',
        'Wtyczki',
      ]) {
        await page
          .getByRole('navigation', { name: 'Sekcje ustawień' })
          .getByRole('button', { name, exact: true })
          .click();
        await expect(page.getByRole('heading', { name, level: 2 })).toBeVisible();
        // Każda zakładka wewnątrz strony (jeśli są) — przełączana myszką.
        const tabs = page.getByRole('main').getByRole('tab');
        const count = await tabs.count();
        for (let t = 0; t < count; t += 1) {
          await tabs.nth(t).click();
          await check(`${name} — zakładka ${t + 1}`);
        }
        // Każda rozwijana sekcja strony w stanie otwartym.
        const toggles = page.getByRole('main').locator('button[aria-expanded="false"]');
        const closed = await toggles.count();
        for (let t = 0; t < Math.min(closed, 6); t += 1) {
          const toggle = toggles.first();
          if (!(await toggle.isVisible())) break;
          await toggle.click();
          await check(`${name} — rozwinięcie ${t + 1}`);
        }
      }
      expect(found, 'wady układu menu i formularzy').toEqual([]);
      expect(errors, 'błędy w konsoli').toEqual([]);
    });

    test(`Kreator agentek krok po kroku — ${style} ${theme}`, async ({ page }) => {
      const errors = watchConsole(page);
      await open(page, style, theme);
      await waitForChat(page);
      await openSettings(page, 'Kreator agentek');
      const found: string[] = [];
      const check = async (label: string) => {
        await page.waitForTimeout(200);
        for (const defect of await layoutDefects(page)) found.push(`${label}: ${defect}`);
      };
      await page
        .getByLabel('Jaka ma być nowa agentka?')
        .fill('Agentka o imieniu Zofia, która porządkuje pobrane pliki.');
      await page.getByRole('button', { name: 'Zaproponuj szkic' }).click();
      await expect(page.getByLabel('Imię', { exact: true })).toHaveValue('Zofia');
      await check('szkic z opisu');
      await page.getByRole('button', { name: 'Pokaż podgląd' }).click();
      const preview = page.getByRole('region', { name: 'Podgląd persony' });
      await expect(preview).toBeVisible();
      await preview.scrollIntoViewIfNeeded();
      await check('podgląd persony');
      await preview.getByRole('button', { name: 'Test na sucho' }).click();
      await expect(preview.getByText('Test na sucho zaliczony')).toBeVisible();
      await check('wynik testu na sucho');
      await preview.getByRole('button', { name: 'Zapisz agentkę' }).click();
      await expect(page.getByText('Zapisano agentkę Zofia.')).toBeVisible();
      const saved = page.getByRole('region', { name: 'Agentki z Kreatora' });
      await saved.scrollIntoViewIfNeeded();
      await check('lista agentek z Kreatora');
      expect(found, 'wady układu Kreatora agentek').toEqual([]);
      expect(errors, 'błędy w konsoli').toEqual([]);
    });

    test(`stany systemowe i okna poboczne — ${style} ${theme}`, async ({ page }) => {
      const errors = watchConsole(page);
      const found: string[] = [];
      for (const scenario of [
        'offline',
        'rate-limited',
        'no-keys',
        'no-mic',
        'disk-low',
        'empty',
      ]) {
        await open(page, style, theme, `&scenario=${scenario}`);
        await waitForChat(page);
        await page.waitForTimeout(200);
        for (const defect of await layoutDefects(page)) found.push(`${scenario}: ${defect}`);
      }
      for (const path of ['/quick.html', '/pill.html']) {
        await open(page, style, theme, '', path);
        await page.waitForTimeout(400);
        for (const defect of await layoutDefects(page)) found.push(`${path}: ${defect}`);
      }
      expect(found, 'wady układu stanów systemowych i okien pobocznych').toEqual([]);
      expect(errors, 'błędy w konsoli').toEqual([]);
    });
  }
}
