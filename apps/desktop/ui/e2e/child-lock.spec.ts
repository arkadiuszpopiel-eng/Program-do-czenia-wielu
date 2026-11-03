import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// Profil Dziecko (plan 50 §4.4, decyzja właściciela 3.11): bez PIN-u rodzica tylko Plan i Ręcznie;
// wyższe tryby i „Max dostęp” z kłódką i podpisem „Poproś rodzica”. Kłódkę otwiera PIN na czas
// (Ustawienia → Uprawnienia), zły PIN liczy próby. Atrapa: `?child=locked` (PIN 2468) albo bez
// PIN-u. Każdy stan w każdym stylu i motywie.

const TITLE = 'Profil Dziecko';
const NOTE = 'Kłódka rodzica — wyższe tryby otwiera PIN rodzica w Ustawienia → Uprawnienia.';

async function open(page: Page, skin: string, theme: 'light' | 'dark', child?: string) {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${skin}&theme=${theme}${child ? `&child=${child}` : ''}`);
  await waitForChat(page);
}

async function openCard(page: Page) {
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Uprawnienia i bezpieczeństwo' })
    .click();
  const card = page.getByRole('region', { name: TITLE });
  await expect(card).toBeVisible();
  return card;
}

for (const skin of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`kłódka w menu trybów — ${skin}, ${theme}`, async ({ page }) => {
      await open(page, skin, theme, 'locked');
      await page.getByRole('button', { name: /^Tryb pracy: Ręcznie/ }).click();
      const menu = page.getByRole('menu', { name: 'Tryb pracy' });
      for (const name of ['Akceptuj edycje', 'Automatycznie', 'Pełny dostęp']) {
        const item = menu.getByRole('menuitemradio', { name: new RegExp(`^${name}`) });
        await expect(item).toHaveAttribute('aria-disabled', 'true');
        await expect(item).toContainText('Poproś rodzica');
      }
      for (const name of ['Plan', 'Ręcznie']) {
        const item = menu.getByRole('menuitemradio', { name: new RegExp(`^${name}`) });
        await expect(item).not.toHaveAttribute('aria-disabled', 'true');
      }
      const max = page.getByRole('switch', { name: 'Max dostęp' });
      await expect(max).toHaveAttribute('aria-disabled', 'true');
      await expectAccessible(page, `kłódka menu ${skin} ${theme}`);
      await expectReadable(page, `kłódka menu ${skin} ${theme}`);
      // Klawisz 3 (Automatycznie) i przełącznik — tylko podpowiedź, bez prośby do Sędziego.
      await menu.getByRole('menuitemradio', { name: /^Ręcznie/ }).focus();
      await page.keyboard.press('3');
      await expect(page.getByRole('status').getByText(NOTE).first()).toBeVisible();
      await expect(menu).toBeVisible();
      // Wyłączony dla dostępności (aria-disabled) — klik myszą nadal tylko podpowiada.
      await max.dispatchEvent('click');
      await expect(max).toHaveAttribute('aria-checked', 'false');
    });

    test(`PIN rodzica otwiera kłódkę na czas — ${skin}, ${theme}`, async ({ page }) => {
      await open(page, skin, theme, 'locked');
      const card = await openCard(page);
      await expect(card.getByRole('status')).toHaveText(
        'Profil Dziecko włączony — kłódka zamknięta.',
      );
      await expectAccessible(page, `profil Dziecko ${skin} ${theme}`);
      await expectReadable(page, `profil Dziecko ${skin} ${theme}`);
      const pin = card.getByLabel('PIN rodzica', { exact: true });
      await pin.fill('1111');
      await card.getByRole('button', { name: 'Otwórz kłódkę' }).click();
      await expect(page.getByText('błędny PIN rodzica')).toBeVisible();
      await expect(card.getByText('Pozostałe próby PIN-u: 4.')).toBeVisible();
      await expect(pin).toHaveValue('');
      await pin.fill('2468');
      await card.getByRole('radio', { name: '1 h' }).click();
      await card.getByRole('button', { name: 'Otwórz kłódkę' }).click();
      await expect(card.getByRole('status')).toContainText('Kłódka otwarta do');
      await expectAccessible(page, `kłódka otwarta ${skin} ${theme}`);
      await card.getByRole('button', { name: 'Zamknij kłódkę' }).click();
      await expect(card.getByRole('status')).toHaveText(
        'Profil Dziecko włączony — kłódka zamknięta.',
      );
    });
  }
}

test('pierwszy PIN, włączenie profilu i blokada po 5 błędnych próbach', async ({ page }) => {
  await open(page, 'classic', 'light');
  const card = await openCard(page);
  await expect(card.getByRole('status')).toHaveText(
    'PIN rodzica nie jest ustawiony — najpierw go ustaw.',
  );
  await card.getByLabel('Nowy PIN (4–8 cyfr)').fill('13579');
  await card.getByLabel('Powtórz nowy PIN').fill('13570');
  await card.getByRole('button', { name: 'Ustaw PIN' }).click();
  await expect(card.getByText('Nowe PIN-y się różnią.')).toBeVisible();
  await card.getByLabel('Nowy PIN (4–8 cyfr)').fill('12');
  await card.getByLabel('Powtórz nowy PIN').fill('12');
  await card.getByRole('button', { name: 'Ustaw PIN' }).click();
  await expect(card.getByText('PIN ma od 4 do 8 cyfr.')).toBeVisible();
  await card.getByLabel('Nowy PIN (4–8 cyfr)').fill('13579');
  await card.getByLabel('Powtórz nowy PIN').fill('13579');
  await card.getByRole('button', { name: 'Ustaw PIN' }).click();
  await expect(card.getByRole('status')).toHaveText(
    'Profil Dziecko wyłączony — tryby pracy bez kłódki.',
  );
  const pin = card.getByLabel('PIN rodzica', { exact: true });
  await pin.fill('13579');
  await card.getByRole('button', { name: 'Włącz profil Dziecko' }).click();
  await expect(card.getByRole('status')).toHaveText('Profil Dziecko włączony — kłódka zamknięta.');
  // Belka i menu po włączeniu: tryb obniżony do „Ręcznie”.
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button', { name: /^Tryb pracy: Ręcznie/ })).toBeVisible();
  await openCard(page);
  for (let i = 0; i < 5; i++) {
    await pin.fill('0000');
    await card.getByRole('button', { name: 'Otwórz kłódkę' }).click();
    await expect(pin).toHaveValue('');
  }
  await expect(card.getByRole('alert')).toContainText('Za dużo błędnych prób');
  await pin.fill('13579');
  await card.getByRole('button', { name: 'Otwórz kłódkę' }).click();
  await expect(page.getByText(/PIN chwilowo zablokowany/)).toBeVisible();
});
