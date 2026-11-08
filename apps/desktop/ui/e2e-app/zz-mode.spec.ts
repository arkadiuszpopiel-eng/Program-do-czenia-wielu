// 29.10 (0.0.76) w PRAWDZIWYM oknie: zmiana trybu pracy z paska pisania dociera do rdzenia i widać
// ją od razu (wcześniej menu po przełączeniu nadal zaznaczało stary tryb). Obniżenie działa bez okna
// Brokera; podniesienia nie testujemy (wymaga potwierdzenia w oknie Brokera). Plik biegnie ostatni —
// po obniżeniu profil instancji testowej zostaje w „Ręcznie” (profil jest tymczasowy, na przebieg).
// Od 3.11 (Z.5) „Plan” obowiązuje tylko otwartą rozmowę — obniżenie globalne sprawdzamy na „Ręcznie”.
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
  test.skip(before.global <= 'L1', 'profil już w „Ręcznie” albo niżej — nie ma czego obniżać');

  await composer.getByRole('button', { name: /^Tryb pracy: / }).click();
  const menu = main.getByRole('menu', { name: 'Tryb pracy' });
  await menu.getByRole('menuitemradio', { name: /Ręcznie/ }).click();
  await expect(main.getByText('Tryb pracy: Ręcznie.')).toBeVisible();
  const chip = composer.getByRole('button', { name: /^Tryb pracy: Ręcznie/ });
  await expect(chip).toBeVisible();
  await expect
    .poll(
      async () =>
        (await call<PermissionsView>(main, 'permissions_get', { sessionId: null })).global,
    )
    .toBe('L1');
  await chip.click();
  await expect(menu.getByRole('menuitemradio', { checked: true })).toContainText('Ręcznie');
  await shot(alfa, '43b-tryb-obnizony');
  await main.keyboard.press('Escape');
});
