import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { expectAccessible, waitForChat } from './helpers';

// P4 (0.0.65): karty rozmów z profilem w panelu Sesje — przełącznik lista ↔ karty zapamiętany na
// tym komputerze, karta z agentkami, profilem, modelem, ostatnią wiadomością, datą i kosztem, to
// samo menu co wiersz listy, strzałki i Enter, wąskie okno (jedna kolumna). Każdy stan w każdym
// stylu i motywie (dostępność i kontrast).

async function open(page: Page, skin = 'classic', theme: 'light' | 'dark' = 'light') {
  await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
  await page.goto(`/?style=${skin}&theme=${theme}`);
  await waitForChat(page);
}

const panel = (page: Page) => page.getByRole('navigation', { name: 'Sesje', exact: true });
const cards = (page: Page) => panel(page).locator('[data-session-card]');

async function showCards(page: Page) {
  const toggle = panel(page).getByRole('button', { name: 'Karty', exact: true });
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  await expect(cards(page).first()).toBeVisible();
}

test('karty: profil rozmowy, przypięcie i koszt; widok zostaje po ponownym uruchomieniu', async ({
  page,
}) => {
  await open(page);
  await expect(panel(page).getByRole('button', { name: 'Lista', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await showCards(page);
  const q3 = panel(page).getByRole('listitem').filter({ hasText: 'Raport Q3' }).first();
  await expect(q3).toContainText('Przypięta');
  // Grupa „Przypięte” — nazwa projektu na karcie, jak w wierszu listy.
  await expect(q3).toContainText('Projekt X');
  await expect(q3).toContainText('claude-opus-5-5');
  await expect(q3).toContainText('Delta:');
  await expect(q3).toContainText('Utworzyłam szkic raport-Q3.docx');
  await expect(q3).toContainText(/Koszt: 1,08/);
  await expect(q3).toContainText('Agentki: Gama, Delta');
  // Ostatnia wiadomość jako zwykły tekst — bez znaczników Markdown.
  await expect(q3.locator('.snippet')).not.toContainText('```');

  await page.reload();
  await waitForChat(page);
  await expect(panel(page).getByRole('button', { name: 'Karty', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(cards(page).first()).toBeVisible();
  await panel(page).getByRole('button', { name: 'Lista', exact: true }).click();
  await expect(cards(page)).toHaveCount(0);
  await expect(panel(page).locator('[data-session-row]').first()).toBeVisible();
});

test('karty: menu akcji takie samo jak w wierszu listy', async ({ page }) => {
  await open(page);
  const row = panel(page).getByRole('listitem').filter({ hasText: 'Raport Q3' }).first();
  await row.hover();
  await row.getByRole('button', { name: /Akcje sesji/ }).click();
  const fromRow = await page.getByRole('menuitem').allTextContents();
  await page.keyboard.press('Escape');
  await showCards(page);
  const card = panel(page).getByRole('listitem').filter({ hasText: 'Raport Q3' }).first();
  await card.hover();
  await card.getByRole('button', { name: /Akcje sesji/ }).click();
  await expect(page.getByRole('menuitem')).toHaveText(fromRow);
  await page.getByRole('menuitem', { name: 'Zmień nazwę' }).click();
  await expect(panel(page).locator('input.rename')).toBeFocused();
  await page.keyboard.press('Escape');
});

test('karty: strzałki przesuwają fokus, Enter otwiera rozmowę', async ({ page }) => {
  await open(page);
  await showCards(page);
  const all = cards(page);
  await all.first().focus();
  await page.keyboard.press('ArrowDown');
  await expect(all.nth(1)).toBeFocused();
  await page.keyboard.press('End');
  await expect(all.last()).toBeFocused();
  await page.keyboard.press('Home');
  await expect(all.first()).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(all.nth(1)).not.toHaveAttribute('aria-current', 'page');
  await page.keyboard.press('Enter');
  await expect(all.nth(1)).toHaveAttribute('aria-current', 'page');
});

test('karty w oknie 400 px: jedna kolumna, bez wychodzenia poza okno', async ({ page }) => {
  await page.setViewportSize({ width: 400, height: 760 });
  await open(page);
  await page.keyboard.press('Control+b');
  const drawer = page.getByRole('dialog', { name: 'Sesje' });
  await expect(drawer).toBeVisible();
  await drawer.getByRole('button', { name: 'Karty', exact: true }).click();
  const list = drawer.locator('[data-session-card]');
  await expect(list.first()).toBeVisible();
  const boxes = await list.evaluateAll((els) =>
    els.slice(0, 3).map((el) => {
      const r = el.getBoundingClientRect();
      return { left: r.left, right: r.right, top: r.top };
    }),
  );
  const lefts = new Set(boxes.map((b) => Math.round(b.left)));
  expect(lefts.size).toBe(1);
  for (const b of boxes) expect(b.right).toBeLessThanOrEqual(401);
  await list.first().focus();
  await page.keyboard.press('ArrowDown');
  await expect(list.nth(1)).toBeFocused();
  await expectAccessible(page, 'karty 400 px');
  await expectReadable(page, 'karty 400 px');
});

for (const skin of ['classic', 'glass', 'next'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`karty rozmów: ${skin}, ${theme}`, async ({ page }) => {
      await open(page, skin, theme);
      await showCards(page);
      await expectAccessible(page, `karty ${skin} ${theme}`);
      await expectReadable(page, `karty ${skin} ${theme}`);
      // Stan z fokusem na karcie (menu widoczne) i z archiwum (znacznik „Archiwum”).
      await panel(page).getByRole('button', { name: 'Pokaż archiwum' }).click();
      await expect(panel(page).getByText('Stare notatki').first()).toBeVisible();
      await cards(page).first().focus();
      await expectAccessible(page, `karty fokus ${skin} ${theme}`);
      await expectReadable(page, `karty fokus ${skin} ${theme}`);
    });
  }
}
