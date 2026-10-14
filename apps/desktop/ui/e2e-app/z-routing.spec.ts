// Dobór modelu w PRAWDZIWEJ aplikacji (audyt 2026-10-14, A-02: polecenia `routing_*` muszą przejść
// przez uprawnienia okna, łącze Tauri i rdzeń). Profil testowy nie ma kont API — strona pokazuje
// ustawienia domyślne, zapis działa, a podgląd ocenia trudność bez wysyłania czegokolwiek do modelu.
import type { Page } from '@playwright/test';
import { expect, expectAccessible, inEverySkin, shot, test } from './alfa';

async function openRouting(page: Page): Promise<void> {
  const nav = page.getByRole('navigation', { name: 'Sekcje ustawień' });
  if (!(await nav.isVisible())) await page.keyboard.press('Control+,');
  await nav.getByRole('button', { name: 'Dobór modelu', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Dobór modelu', level: 2 })).toBeVisible();
}

test('dobór modelu: ustawienia z rdzenia, zapis i podgląd na przykładzie', async ({ alfa }) => {
  const { main } = alfa;
  await openRouting(main);
  const enabled = main.getByRole('switch', { name: 'Dobieraj model do trudności zadania' });
  await expect(enabled).toBeChecked();
  await expect(main.getByRole('radio', { name: /Zrównoważenie/ })).toBeChecked();

  // Zapis przez rdzeń: strategia „Jakość”, potem powrót do domyślnej.
  await main.getByRole('radio', { name: /Jakość/ }).click();
  await expect(main.getByRole('radio', { name: /Jakość/ })).toBeChecked();
  const saved = await main.evaluate(() => {
    const ipc = window as unknown as {
      __TAURI_INTERNALS__: {
        invoke: (cmd: string) => Promise<{ settings: { strategy: string } }>;
      };
    };
    return ipc.__TAURI_INTERNALS__.invoke('routing_get').then((r) => r.settings.strategy);
  });
  expect(saved).toBe('quality');
  await main.getByRole('radio', { name: /Zrównoważenie/ }).click();
  await expect(main.getByRole('radio', { name: /Zrównoważenie/ })).toBeChecked();

  // Podgląd: ocena trudności z rdzenia (bez modelu — profil testowy nie ma kont).
  await main.getByLabel('Przykładowa prośba').fill('Która jest godzina?');
  await main.getByRole('button', { name: 'Sprawdź', exact: true }).click();
  await expect(main.locator('.result')).toContainText('Zadanie łatwe: krótka, prosta prośba');
  await expectAccessible(alfa, 'dobór modelu');
  await shot(alfa, '30-dobor-modelu');
  await inEverySkin(main, async (skin, theme) => {
    await expect(main.getByRole('heading', { name: 'Dobór modelu', level: 2 })).toBeVisible();
    await shot(alfa, `31-dobor-modelu-${skin}-${theme}`);
  });
});
