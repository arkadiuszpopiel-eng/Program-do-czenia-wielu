// Informacja „rozmawiasz ze sztuczną inteligencją” (AI Act art. 50, zadanie C2.9): przy pierwszym
// uruchomieniu (szyna wprowadzenia) i stale pod polem wpisywania — czytelna w każdym stylu i motywie.
import { expect, test } from '@playwright/test';
import { expectReadable } from './contrast';
import { openApp, waitForChat } from './helpers';

const NOTICE = 'Rozmawiasz ze sztuczną inteligencją — sprawdzaj ważne informacje';

test('pierwsze uruchomienie mówi, że Alfa to AI', async ({ page }) => {
  await openApp(page, { scenario: 'first-run' });
  await expect(page.getByText(/asystentka oparta na sztucznej inteligencji \(AI\)/)).toBeVisible();
});

for (const skin of ['classic', 'glass', 'next', 'junior'] as const) {
  for (const theme of ['light', 'dark'] as const) {
    test(`stała informacja pod polem wpisywania: ${skin}, ${theme}`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
      await page.goto(`/?style=${skin}&theme=${theme}`);
      await waitForChat(page);
      await expect(page.getByText(NOTICE)).toBeVisible();
      await expectReadable(page, `informacja AI ${skin} ${theme}`);
    });
  }
}
