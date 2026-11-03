import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

// „Max dostęp” (decyzja właściciela 3.11, plan 50 §8.1): czerwony przełącznik na dole menu
// „Tryb pracy” z zasięgiem lokalne / chmura / wszystkie. Włączenie i zmiana zasięgu — karta
// w oknie Sędziego Cieni (stan bez zmian do zgody), wyłączenie od razu. Każdy styl i motyw.
for (const style of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`Max dostęp wyłączony — ${style} ${theme}`, async ({ page }) => {
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      await expect(page.locator('header.titlebar .badge')).toHaveCount(0);
      await page.getByRole('button', { name: /^Tryb pracy: Automatycznie/ }).click();
      const max = page.getByRole('switch', { name: 'Max dostęp' });
      await expect(max).toHaveAttribute('aria-checked', 'false');
      const scopes = page.getByRole('radiogroup', { name: 'Zasięg Max dostępu' });
      await expect(scopes.getByRole('radio')).toHaveCount(3);
      await expect(scopes.getByRole('radio', { checked: true })).toHaveText('Lokalne');
      await expectAccessible(page, `Max dostęp wyłączony ${style} ${theme}`);

      // Strzałka w prawo wybiera „Chmura”; włączenie czeka na okno Sędziego.
      await scopes.getByRole('radio', { checked: true }).focus();
      await page.keyboard.press('ArrowRight');
      await expect(scopes.getByRole('radio', { checked: true })).toHaveText('Chmura');
      await expect(scopes.getByRole('radio', { checked: true })).toBeFocused();
      await max.click();
      await expect(
        page.getByText('Potwierdź „Max dostęp” (Chmura) w oknie Sędziego Cieni'),
      ).toBeVisible();
      await expect(max).toHaveAttribute('aria-checked', 'false');
      await expect(page.locator('header.titlebar .badge')).toHaveCount(0);
    });

    test(`Max dostęp włączony — ${style} ${theme}`, async ({ page }) => {
      await openApp(page, { theme, path: `/?style=${style}&max=all` });
      await waitForChat(page);
      // Znak w belce: czerwony, z zasięgiem w opisie; klik otwiera menu trybów.
      const badge = page.getByRole('button', { name: /^Włączony Max dostęp \(Wszystkie\)/ });
      await expect(badge).toBeVisible();
      await badge.click();
      const max = page.getByRole('switch', { name: 'Max dostęp' });
      await expect(max).toHaveAttribute('aria-checked', 'true');
      await expect(page.getByText('Włączony: Wszystkie — agentki nie pytają o nic.')).toBeVisible();
      await expectAccessible(page, `Max dostęp włączony ${style} ${theme}`);

      // Zmiana zasięgu włączonego przełącznika — nowa karta, stan bez zmian.
      const scopes = page.getByRole('radiogroup', { name: 'Zasięg Max dostępu' });
      await scopes.getByRole('radio', { name: 'Lokalne' }).click();
      await expect(
        page.getByText('Potwierdź „Max dostęp” (Lokalne) w oknie Sędziego Cieni'),
      ).toBeVisible();
      await expect(max).toHaveAttribute('aria-checked', 'true');

      // Wyłączenie od razu: przełącznik i znak w belce znikają.
      await max.click();
      await expect(page.getByText('Max dostęp wyłączony')).toBeVisible();
      await expect(max).toHaveAttribute('aria-checked', 'false');
      await expect(badge).toHaveCount(0);
    });
  }
}
