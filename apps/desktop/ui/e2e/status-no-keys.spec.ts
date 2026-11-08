import { expect, test } from '@playwright/test';
import { openApp, waitForChat } from './helpers';

// Bez kluczy API działa tylko profil lokalny: belka, okienko stanu i pasek pisania pokazują ten
// sam profil — „Lokalny” — a nie zapisaną w ustawieniach „Hybrydę” (przegląd w WebView2 28.10).
// Zaznaczony wybór ma obwódkę widoczną w każdym stylu (w jasnym Szkle tła prawie się zlewały).
for (const style of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`stan bez kluczy: profil Lokalny wszędzie — ${style} ${theme}`, async ({ page }) => {
      await openApp(page, {
        scenario: 'no-keys',
        theme,
        path: `/?scenario=no-keys&style=${style}`,
      });
      await waitForChat(page);
      await page.getByRole('button', { name: /^Stan: .*profil Lokalny/ }).click();
      const details = page.getByRole('dialog', { name: 'Stan Alfy' });
      await expect(details).toContainText('Sieć działa; modele w chmurze wymagają klucza API.');
      const local = details.getByRole('radio', { name: 'Lokalny' });
      await expect(local).toHaveAttribute('aria-checked', 'true');
      await expect(local).toHaveAttribute('tabindex', '0');
      await expect(local).toBeEnabled();
      for (const name of ['Hybryda', 'Chmura']) {
        const other = details.getByRole('radio', { name });
        await expect(other).toHaveAttribute('aria-checked', 'false');
        await expect(other).toBeDisabled();
      }
      await expect(local).toHaveCSS('outline-style', 'solid');
      await page.keyboard.press('Escape');
      await expect(page.locator('form.composer')).toContainText('Lokalny');
    });
  }
}
