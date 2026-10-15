// Serie 3 i 4 (0.0.62) na PRAWDZIWYM rdzeniu: nowe polecenia bez modelu w testerze (jednostki, daty,
// lata przestępne, liczby słownie, procenty, wybór z listy, liczenie znaków) i zapis trybu skupienia
// w układzie okna (`LayoutPrefs.focus`, starszy zapis bez pola = wyłączony).
import { expect, test } from './alfa';
import { call } from './ipc-call';

interface QuickTest {
  source: string;
  name: string | null;
  reply: string | null;
}

interface Bootstrap {
  layout: { focus: boolean } | null;
}

test('nowe polecenia bez modelu i tryb skupienia na prawdziwym rdzeniu', async ({ alfa }) => {
  const { main } = alfa;
  const tester = (message: string) => call<QuickTest>(main, 'quick_commands_test', { message });
  for (const [message, name, reply] of [
    ['Ile to 5 km w milach?', 'przeliczanie jednostek', '5 km = **3,10686 mi**'],
    [
      'Jaki dzień tygodnia był 1 stycznia 2000?',
      'rachunek na datach',
      '1 stycznia 2000 to **sobota**.',
    ],
    ['Ile dni ma luty 2028?', 'rachunek na datach', 'Luty 2028 ma **29 dni**.'],
    [
      'Zapisz 1999,99 zł słownie',
      'liczby słownie i rzymskie',
      '1999,99 zł = **tysiąc dziewięćset dziewięćdziesiąt dziewięć złotych 99/100**',
    ],
    ['2026 rzymskimi', 'liczby słownie i rzymskie', '2026 = **MMXXVI**'],
    ['Ile procent to 25 z 200?', 'rachunek', '25 z 200 to **12,5%**'],
    [
      'Policz znaki: Ala ma kota',
      'liczenie znaków i słów',
      '**11** znaków (9 bez spacji, 9 liter), **3** słowa.',
    ],
    ['Wybierz jedno: pizza, sushi', 'losowanie', 'Wybieram: **sushi**.'],
  ] as const) {
    const result = await tester(message);
    expect(result.source, message).toBe('builtin');
    expect(result.name, message).toBe(name);
    expect(result.reply, message).toBe(reply);
  }
  expect((await tester('Ile kosztuje 5 kg ziemniaków?')).source).toBe('model');

  const layout = { left_width: 280, right_width: 360, left_collapsed: false, sessions: {} };
  await call(main, 'app_save_layout', { layout: { ...layout, focus: true } });
  expect((await call<Bootstrap>(main, 'app_bootstrap')).layout?.focus).toBe(true);
  await call(main, 'app_save_layout', { layout });
  expect((await call<Bootstrap>(main, 'app_bootstrap')).layout?.focus).toBe(false);
});
