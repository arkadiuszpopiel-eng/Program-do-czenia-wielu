// Z.8 (plan 50 §5) w PRAWDZIWYM oknie: profil Dziecko przez rdzeń i Sędziego Cieni. PIN rodzica
// (przez IPC — szybciej i bez zależności od układu karty), włączenie profilu, w menu „Tryb pracy”
// wyższe tryby z kłódką i „Poproś rodzica”, przełącznik „Max dostęp” wyłączony, znak w belce.
// Na końcu profil WYŁĄCZONY PIN-em — następny plik (zz-mode) obniża tryb bez kłódki. Profil
// instancji testowej jest tymczasowy (na przebieg); przy ustawionym już PIN-ie test się pomija.
import { expect, shot, test } from './alfa';
import { call } from './ipc-call';

interface ChildLock {
  pin_set: boolean;
  enabled: boolean;
  open: boolean;
  tamper_resistant: boolean;
  unknown: boolean;
}

const PIN = '2468';

test('profil Dziecko: kłódka w menu trybów, wyłączenie PIN-em', async ({ alfa }) => {
  const { main } = alfa;
  const back = main.getByRole('button', { name: 'Wróć do rozmowy' });
  if (await back.isVisible()) await back.click();
  const composer = main.locator('form.composer');
  await expect(composer).toBeVisible({ timeout: 30_000 });
  const before = await call<ChildLock>(main, 'permissions_child_lock', { change: null });
  expect(before.unknown).toBe(false);
  test.skip(before.pin_set, 'PIN rodzica już ustawiony w tym profilu — nie znamy go');

  await call(main, 'permissions_child_lock', { change: { op: 'set_pin', old: null, new: PIN } });
  const on = await call<ChildLock>(main, 'permissions_child_lock', {
    change: { op: 'enable', pin: PIN, on: true },
  });
  expect(on.enabled && !on.open).toBe(true);
  try {
    // Menu „Tryb pracy” czyta stan z Sędziego przy każdym otwarciu.
    await composer.getByRole('button', { name: /^Tryb pracy: / }).click();
    const menu = main.getByRole('menu', { name: 'Tryb pracy' });
    await expect(
      main.getByRole('button', { name: /^Profil Dziecko — kłódka zamknięta/ }),
    ).toBeVisible({ timeout: 15_000 });
    for (const name of ['Akceptuj edycje', 'Automatycznie', 'Pełny dostęp']) {
      const item = menu.getByRole('menuitemradio', { name: new RegExp(`^${name}`) });
      await expect(item).toHaveAttribute('aria-disabled', 'true');
      await expect(item).toContainText('Poproś rodzica');
    }
    await expect(main.getByRole('switch', { name: 'Max dostęp' })).toHaveAttribute(
      'aria-disabled',
      'true',
    );
    await shot(alfa, '52a-klodka-dziecko');
    // Klawisz 4 (Automatycznie) — tylko podpowiedź; Sędzia nie dostaje prośby.
    await main.keyboard.press('4');
    const view = await call<{ global: string }>(main, 'permissions_get', { sessionId: null });
    expect(view.global <= 'L1').toBe(true);
    await main.keyboard.press('Escape');
  } finally {
    const off = await call<ChildLock>(main, 'permissions_child_lock', {
      change: { op: 'enable', pin: PIN, on: false },
    });
    expect(off.enabled).toBe(false);
  }
});
