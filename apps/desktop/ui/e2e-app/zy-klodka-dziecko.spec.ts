// Z.8 (plan 50 §5) w PRAWDZIWYM oknie: profil Dziecko przez rdzeń i Sędziego Cieni. PIN rodzica
// (przez IPC — szybciej i bez zależności od układu karty), włączenie profilu, w menu „Tryb pracy”
// wyższe tryby z kłódką i „Poproś rodzica”, przełącznik „Max dostęp” wyłączony, znak w belce.
// Operacje na kłódce trafiają do dziennika rozmowy (Z.7). Na końcu profil WYŁĄCZONY PIN-em — następny plik (zz-mode) obniża tryb bez kłódki. Profil
// instancji testowej jest tymczasowy (na przebieg); przy ustawionym już PIN-ie test się pomija.
import { expect, shot, test } from './alfa';
import { call, refused } from './ipc-call';

interface ChildLock {
  pin_set: boolean;
  enabled: boolean;
  open: boolean;
  tamper_resistant: boolean;
  unknown: boolean;
}

const PIN = '2468';

// Nazwane kroki i znaczniki nie zawierają argumentów IPC ani PIN-u. Sprzątanie może
// przesłonić pierwotne oczekiwanie po timeoutcie; ostatni znacznik wskazuje rozpoczętą operację.
async function step<T>(name: string, run: () => Promise<T>): Promise<T> {
  return test.step(name, async () => {
    console.info(`[child-lock] ${name}: start`);
    try {
      const result = await run();
      console.info(`[child-lock] ${name}: ok`);
      return result;
    } catch (error) {
      console.info(`[child-lock] ${name}: error`);
      throw error;
    }
  });
}

test('profil Dziecko: kłódka w menu trybów, wyłączenie PIN-em', async ({ alfa }) => {
  const { main } = alfa;
  const back = main.getByRole('button', { name: 'Wróć do rozmowy' });
  if (await back.isVisible()) await back.click();
  const composer = main.locator('form.composer');
  await expect(composer).toBeVisible({ timeout: 30_000 });
  const before = await step('odczyt stanu początkowego', () =>
    call<ChildLock>(main, 'permissions_child_lock', { change: null }),
  );
  expect(before.unknown).toBe(false);
  test.skip(before.pin_set, 'PIN rodzica już ustawiony w tym profilu — nie znamy go');

  await step('ustawienie testowego PIN-u', () =>
    call(main, 'permissions_child_lock', { change: { op: 'set_pin', old: null, new: PIN } }),
  );
  const on = await step('włączenie kłódki', () =>
    call<ChildLock>(main, 'permissions_child_lock', {
      change: { op: 'enable', pin: PIN, on: true },
    }),
  );
  expect(on.enabled && !on.open).toBe(true);
  try {
    // Menu „Tryb pracy” czyta stan z Sędziego przy każdym otwarciu.
    await step('otwarcie menu trybów', () =>
      composer.getByRole('button', { name: /^Tryb pracy: / }).click(),
    );
    await step('sprawdzenie blokad w menu trybów', async () => {
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
    });
    await step('zrzut kłódki w menu', () => shot(alfa, '52a-klodka-dziecko'));
    // Klawisz 4 (Automatycznie) — tylko podpowiedź; Sędzia nie dostaje prośby.
    await main.keyboard.press('4');
    const view = await step('odczyt poziomu po skrócie wyższego trybu', () =>
      call<{ global: string }>(main, 'permissions_get', { sessionId: null }),
    );
    expect(view.global <= 'L1').toBe(true);
    await main.keyboard.press('Escape');
    // Max dostęp przy zamkniętej kłódce — odmowa Sędziego od razu, bez karty w oknie zgód.
    await step('odmowa Max dostępu przy zamkniętej kłódce', () =>
      refused(main, 'permissions_request_max_access', { scope: 'local' }),
    );

    // Z.7 „do końca rozmowy”: kłódka otwarta tylko dla aktywnej rozmowy; inna rozmowa ją zamyka.
    const { before, a, b } = await step('utworzenie dwóch rozmów próby kłódki', async () => ({
      before: await call<{ active_session_id: string | null }>(main, 'app_bootstrap', {}),
      a: await call<{ id: string }>(main, 'sessions_create', { template: 'empty' }),
      b: await call<{ id: string }>(main, 'sessions_create', { template: 'empty' }),
    }));
    try {
      await step('przełączenie na rozmowę A', () =>
        call(main, 'app_set_active_session', { sessionId: a.id }),
      );
      const open = await step('otwarcie kłódki do końca rozmowy A', () =>
        call<ChildLock & { conversation: boolean; conversation_id?: string }>(
          main,
          'permissions_child_lock',
          { change: { op: 'open', pin: PIN, minutes: null, session_id: a.id } },
        ),
      );
      expect(open.open && open.conversation).toBe(true);
      expect(open.conversation_id).toBe(a.id);
      await step('przełączenie na rozmowę B zamyka kłódkę', () =>
        call(main, 'app_set_active_session', { sessionId: b.id }),
      );
      const after = await step('odczyt zamknięcia po zmianie rozmowy', () =>
        call<ChildLock & { conversation: boolean }>(main, 'permissions_child_lock', {
          change: null,
        }),
      );
      expect(after.open || after.conversation).toBe(false);
    } finally {
      await step('przywrócenie poprzedniej rozmowy', () =>
        call(main, 'app_set_active_session', { sessionId: before.active_session_id }),
      );
    }
  } finally {
    const off = await step('wyłączenie kłódki w sprzątaniu', () =>
      call<ChildLock>(main, 'permissions_child_lock', {
        change: { op: 'enable', pin: PIN, on: false },
      }),
    );
    expect(off.enabled).toBe(false);
  }
  // Wyłączenie Max dostępu (zawężenie) działa zawsze, bez okna zgód.
  await call(main, 'permissions_request_max_access', { scope: null });
  // Z.7: operacje na kłódce w dzienniku aktywnej rozmowy (wpis „audit”), bez PIN-u.
  const boot = await call<{ active_session_id: string | null }>(main, 'app_bootstrap', {});
  if (boot.active_session_id) {
    const events = await call<{ title: string }[]>(main, 'timeline_list', {
      sessionId: boot.active_session_id,
      filter: { kinds: ['audit'], min_level: 'debug' },
    });
    const titles = events.map((e) => e.title);
    expect(titles).toContain('Profil Dziecko: włączenie profilu Dziecko');
    expect(titles).toContain('Profil Dziecko: wyłączenie profilu Dziecko');
    expect(titles.join(' ')).not.toContain(PIN);
  }
});
