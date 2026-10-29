import { expect, test } from '@playwright/test';
import { expectAccessible, openApp, waitForChat } from './helpers';

// Kula stanu rozmowy w panelu „Głos”: stan słowem, kto mówi i to, co Alfa właśnie rozpoznaje.
for (const style of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`kula stanu rozmowy przechodzi przez stany — ${style} ${theme}`, async ({ page }) => {
      await openApp(page, { theme, path: `/?style=${style}` });
      await waitForChat(page);
      await page.keyboard.press('Alt+6');
      const orb = page.getByTestId('voice-orb');
      await expect(orb).toHaveAttribute('data-state', 'off');
      await expect(orb).toContainText('Mikrofon wyłączony');
      await expect(orb).toContainText('Włącz rozmowę, żeby mówić do Alfy.');
      await expectAccessible(page, `kula: wyłączona ${style} ${theme}`);
      // Pasek nad polem pisania pojawia się dopiero, gdy tor głosu coś robi.
      const live = page.getByTestId('voice-live');
      await expect(live).toHaveCount(0);
      await page.getByRole('button', { name: 'Włącz rozmowę', exact: true }).click();
      // Użytkownik mówi: stan „Słyszy Cię” i rosnący transkrypt.
      await expect(orb).toContainText(/Słyszy Cię.*„Co/);
      await expect(live).toContainText(/Słyszy Cię.*„Co/);
      // Agentka mówi: podpis z imieniem; transkrypt znika.
      await expect(orb).toContainText(/Agentka mówi\s*Mówi Alfa — możesz przerwać głosem\.\s*$/);
      await expect(live).toHaveText(/Mówi Alfa/);
      await expect(orb).toHaveAttribute('data-state', 'listening', { timeout: 10_000 });
      await expect(live).toHaveCount(0);
      await expect(orb).toContainText('Czeka, aż się odezwiesz.');
      await expectAccessible(page, `kula: słucha ${style} ${theme}`);
      await page.getByRole('button', { name: 'Wyłącz rozmowę', exact: true }).click();
      await expect(orb).toHaveAttribute('data-state', 'off');
    });
  }
}
