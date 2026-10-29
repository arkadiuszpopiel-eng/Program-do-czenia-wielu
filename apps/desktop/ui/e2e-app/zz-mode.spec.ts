// 29.10 (0.0.76) w PRAWDZIWYM oknie: zmiana trybu pracy z paska pisania dociera do rdzenia i widać
// ją od razu (wcześniej menu po przełączeniu nadal zaznaczało stary tryb). Obniżenie działa bez okna
// Brokera; podniesienia nie testujemy (wymaga potwierdzenia w oknie Brokera). Plik biegnie ostatni —
// po obniżeniu profil instancji testowej zostaje w „Tylko podgląd” (profil jest tymczasowy, na przebieg).
import { expect, shot, test } from './alfa';
import { call } from './ipc-call';

interface PermissionsView {
  global: string;
}

test('tryb pracy: obniżenie z paska pisania zmienia tryb w rdzeniu i na żetonie', async ({
  alfa,
}) => {
  const { main } = alfa;
  const back = main.getByRole('button', { name: 'Wróć do rozmowy' });
  if (await back.isVisible()) await back.click();
  const composer = main.locator('form.composer');
  await expect(composer).toBeVisible({ timeout: 30_000 });
  const before = await call<PermissionsView>(main, 'permissions_get', { sessionId: null });
  test.skip(before.global === 'L0', 'profil już w „Tylko podgląd” — nie ma czego obniżać');

  await composer.getByRole('button', { name: /^Tryb pracy: / }).click();
  const menu = main.getByRole('menu', { name: 'Tryb pracy' });
  await menu.getByRole('menuitemradio', { name: /Tylko podgląd/ }).click();
  await expect(main.getByText('Tryb pracy: Tylko podgląd.')).toBeVisible();
  const chip = composer.getByRole('button', { name: /^Tryb pracy: Tylko podgląd/ });
  await expect(chip).toBeVisible();
  await expect
    .poll(
      async () =>
        (await call<PermissionsView>(main, 'permissions_get', { sessionId: null })).global,
    )
    .toBe('L0');
  await chip.click();
  await expect(menu.getByRole('menuitemradio', { checked: true })).toContainText('Tylko podgląd');
  await shot(alfa, '43b-tryb-obnizony');
  await main.keyboard.press('Escape');
});
