import { expect, test, type Page } from '@playwright/test';
import { expectReadable } from './contrast';
import { waitForChat } from './helpers';

// Zgłoszenia właściciela (2026-10-09): pasek LED wchodził na elementy belki, brak ekranu
// ładowania, ikony agentek do solidnej poprawy (mruganie, mini awatary w rytm mowy), zakładka
// głosu do rozbudowy (głos sztywny i niemiły).

const SKINS = ['classic', 'glass', 'next'] as const;
const THEMES = ['light', 'dark'] as const;
const CAST = 'cast=speaking,working,waiting_approval,idle';

async function open(page: Page, query = '', motion: 'reduce' | 'no-preference' = 'reduce') {
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: motion });
  await page.goto(`/${query}`);
  await waitForChat(page);
}

const cast = (page: Page) => page.getByRole('group', { name: 'Obsada agentek' });

test('ekran ładowania jest w stronie od pierwszej klatki i znika po starcie', async ({ page }) => {
  const html = await (await page.request.get('/')).text();
  expect(html).toContain('id="alfa-boot"');
  expect(html).toContain('Uruchamianie…');
  // Styl ekranu jest osobnym plikiem wczytywanym przed kodem programu; strona nie ma bloku
  // <style> (w oknie programu wyłączyłby style nadawane elementom — CSP z jednorazowym znacznikiem).
  expect(html).not.toContain('<style');
  expect(html.indexOf('href="/boot.css"')).toBeLessThan(html.indexOf('<script'));
  const css = await (await page.request.get('/boot.css')).text();
  expect(css).toContain('#alfa-boot');
  await open(page);
  await expect(page.locator('#alfa-boot')).toHaveCount(0);
  await expect(page.locator('#alfa-composer')).toBeVisible();
});

for (const skin of SKINS) {
  for (const theme of THEMES) {
    test(`belka: pasek LED ma własny tor, nic na niego nie wchodzi — ${skin}/${theme}`, async ({
      page,
    }) => {
      await open(page, `?style=${skin}&theme=${theme}&${CAST}`, 'no-preference');
      const gap = await page.evaluate(() => {
        const bar = document.querySelector('header.titlebar');
        const led = bar?.querySelector('.led');
        if (!bar || !led) return { gap: -1, worst: 'brak paska' };
        const top = led.getBoundingClientRect().top;
        let gap = Infinity;
        let worst = '';
        // Kapsuły, przyciski i awatary (bez przycisków okna, które sięgają do krawędzi belki).
        const parts = bar.querySelectorAll(
          '.brand, .crumbs, .cast, .gui, .mic, .badge, .icon-btn, .avatar',
        );
        for (const el of parts) {
          const box = el.getBoundingClientRect();
          if (box.width === 0) continue;
          if (top - box.bottom < gap) {
            gap = top - box.bottom;
            worst = el.className.toString().split(' ')[0] ?? '';
          }
        }
        return { gap, worst };
      });
      expect(gap.gap, `odstęp nad paskiem LED (najbliżej: ${gap.worst})`).toBeGreaterThanOrEqual(2);
      // Pasek ma 3 px i leży na dolnej krawędzi belki.
      const led = await page.locator('header.titlebar .led').boundingBox();
      const bar = await page.locator('header.titlebar').boundingBox();
      expect(led?.height).toBe(3);
      expect(Math.round((led?.y ?? 0) + (led?.height ?? 0))).toBeLessThanOrEqual(
        Math.round((bar?.y ?? 0) + (bar?.height ?? 0)),
      );
      await expectReadable(page, 'header.titlebar');
    });
  }
}

test('awatary: mówiąca i pracująca agentka dostają twarz, gotowa zostaje literą', async ({
  page,
}) => {
  await open(page, `?${CAST}`, 'no-preference');
  const buttons = cast(page).getByRole('button');
  // α mówi: twarz z ustami w ruchu i pulsujący pierścień.
  const alfa = buttons.nth(0).locator('.avatar');
  await expect(alfa).toHaveClass(/speaking/);
  await expect(alfa.locator('svg.mini .mouth.talk')).toHaveCount(1);
  await expect(alfa.locator('.eye')).toHaveCount(2);
  const mouth = alfa.locator('.mouth.talk');
  expect(await mouth.evaluate((el) => getComputedStyle(el).animationName)).not.toBe('none');
  // β pracuje: twarz i krążący łuk; δ gotowa: sama litera.
  const beta = buttons.nth(1).locator('.avatar');
  await expect(beta.locator('svg.mini')).toHaveCount(1);
  await expect(beta.locator('.orbit')).toHaveCount(1);
  expect(await beta.locator('.eyes').evaluate((el) => getComputedStyle(el).animationName)).toMatch(
    /blink/,
  );
  const delta = buttons.nth(3).locator('.avatar');
  await expect(delta.locator('svg.mini')).toHaveCount(0);
  await expect(delta.locator('.glyph')).toHaveText('δ');
  await expect(buttons.nth(0)).toHaveAccessibleName(/^Alfa · .* · mówi/);

  // Styl „same litery”: żadnych twarzy, stan dalej widać po pierścieniu.
  await open(page, `?${CAST}&avatars=letters`);
  await expect(cast(page).locator('svg.mini')).toHaveCount(0);
  await expect(cast(page).getByRole('button').nth(0).locator('.avatar')).toHaveClass(/speaking/);
});

// Właściciel 27.10: „z ust mogą wylatywać literki”. Litery biorą się z końcówki pisanej
// odpowiedzi; są ozdobą (poza drzewem dostępności) i znikają przy ograniczonym ruchu.
test('awatary: z ust piszącej agentki wylatują litery jej odpowiedzi', async ({ page }) => {
  await open(page, '?scenario=slow-stream', 'no-preference');
  const feed = page.getByRole('feed', { name: 'Wiadomości' });
  const composer = page.locator('#alfa-composer');
  await composer.fill('Opowiedz bajkę o smoku.');
  await composer.press('Enter');
  const speaking = feed.locator('.avatar.speaking').last();
  await expect(speaking).toBeVisible();
  const fly = speaking.locator('.fly');
  await expect(fly.first()).toBeAttached({ timeout: 5_000 });
  await expect(fly.first()).toHaveAttribute('aria-hidden', 'true');
  // Odpowiedź atrapy zaczyna się od „Jasne. Rozpisałam…” — litery pochodzą z jej tekstu.
  const text = (await feed.locator('article').last().innerText()).toLowerCase();
  const ch = await fly.first().getAttribute('data-ch');
  expect(ch && text.includes(ch)).toBe(true);

  await open(page, '?scenario=slow-stream', 'reduce');
  await composer.fill('Jeszcze raz.');
  await composer.press('Enter');
  await expect(feed.locator('.avatar.speaking').last()).toBeVisible();
  await page.waitForTimeout(1_000);
  await expect(feed.locator('.fly')).toHaveCount(0);
});

test('głos: strojenie mowy przy głosach agentek, odsłuch rusza ustami w rytm poziomu', async ({
  page,
}) => {
  await open(page, '', 'no-preference');
  await page.keyboard.press('Control+,');
  await page
    .getByRole('navigation', { name: 'Sekcje ustawień' })
    .getByRole('button', { name: 'Głos', exact: true })
    .click();
  const voices = page.locator('section[aria-labelledby="voice-voices"]');
  await expect(voices.getByRole('heading', { name: 'Głosy agentek' })).toBeVisible();
  for (const label of [
    'Styl mówienia',
    'Tempo mowy',
    'Pauza między zdaniami',
    'Różnice między głosami agentek',
  ]) {
    // Każde ustawienie jest raz — przy głosach, nie drugi raz na liście ogólnej.
    const inVoices = await voices.getByText(label, { exact: true }).count();
    expect(inVoices).toBeGreaterThan(0);
    await expect(page.locator('section.content').getByText(label, { exact: true })).toHaveCount(
      inVoices,
    );
  }
  await expect(voices.getByText('Domyślnie: Naturalny (zalecany)')).toBeVisible();
  await expect(voices.getByText(/Domyślnie: 200 ms/)).toBeVisible();

  // Duże awatary mają twarz zawsze; po „Odsłuchaj” usta Bety idą za poziomem mowy.
  const beta = voices.locator('li.voice').nth(1).locator('.avatar');
  await expect(beta.locator('svg.mini')).toHaveCount(1);
  await expect(beta).not.toHaveClass(/speaking/);
  await voices.getByRole('button', { name: 'Odsłuchaj głos: Beta' }).click();
  await expect(beta).toHaveClass(/speaking/);
  await expect(beta).toHaveClass(/measured/);
  const openings = new Set<string>();
  for (let i = 0; i < 12; i++) {
    openings.add(await beta.evaluate((el) => (el as HTMLElement).style.getPropertyValue('--open')));
    await page.waitForTimeout(60);
  }
  expect(openings.size, 'usta zmieniają otwarcie w rytm poziomu').toBeGreaterThan(3);
  // W belce ta sama agentka jest oznaczona jako mówiąca.
  await expect(cast(page).getByRole('button').nth(1)).toHaveAccessibleName(/^Beta · .* · mówi/);
  // Po odsłuchu wraca spokój.
  await expect(beta).not.toHaveClass(/speaking/, { timeout: 6_000 });
  await expectReadable(page, 'section[aria-labelledby="voice-voices"]');
});
