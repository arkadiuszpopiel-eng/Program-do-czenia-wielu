// Etap K.3 (styl „Junior”, decyzje właściciela 29.10) w PRAWDZIWYM oknie (WebView2, CSP, rdzeń):
// wybór stylu w Wyglądzie powiększa interfejs ×1,15, propozycja „Włącz” zapisuje w rdzeniu
// wolniejszą mowę, dźwięki i uproszczony ekran; uproszczony ekran chowa przyciski, ale wskaźniki
// i tryb pracy zostają; rdzeń odrzuca złe wartości nowych ustawień. Bez dźwięku (bez czytania).
import { expect, expectAccessible, shot, test } from './alfa';
import { call, refused } from './ipc-call';
import { pickOption } from '../e2e/helpers';

const KEYS = ['ui.style', 'ui.simple', 'voice.tts_speed', 'notify.earcons'] as const;

test('Junior i uproszczony ekran w WebView2: skala, propozycja, wskaźniki, walidacja rdzenia', async ({
  alfa,
}) => {
  const { main } = alfa;
  const values = await call<Record<string, unknown>>(main, 'settings_values');
  const before = Object.fromEntries(KEYS.map((k) => [k, values[k]]));
  try {
    // Rdzeń przyjmuje tylko wartości ze schematu (lista stylów, przełącznik).
    await refused(main, 'settings_set', { key: 'ui.style', value: 'nieistniejacy' });
    await refused(main, 'settings_set', { key: 'ui.simple', value: 'tak' });

    const back = main.getByRole('button', { name: 'Wróć do rozmowy' });
    if (await back.isVisible()) await back.click();
    await expect(main.locator('#alfa-composer')).toBeVisible({ timeout: 30_000 });
    await main.keyboard.press('Control+,');
    await main
      .getByRole('navigation', { name: 'Sekcje ustawień' })
      .getByRole('button', { name: 'Wygląd' })
      .click();
    const html = main.locator('html');
    const zoom = () => html.evaluate((el) => Number(getComputedStyle(el).zoom));
    const start = await zoom();
    await pickOption(main.getByRole('combobox', { name: 'Styl', exact: true }), 'junior');
    await expect(html).toHaveAttribute('data-skin', 'junior');
    await expect.poll(zoom).toBeCloseTo(start * 1.15, 2);
    await expect(main.getByText(/^Styl Junior: włączyć też wolniejszą mowę/)).toBeVisible();
    await shot(alfa, 'k3a-junior-propozycja');
    await main.getByRole('button', { name: 'Włącz', exact: true }).click();
    await expect(
      main.getByText('Mowa w tempie 85 %, dźwięki sygnałowe i uproszczony ekran włączone.'),
    ).toBeVisible();
    const saved = await call<Record<string, unknown>>(main, 'settings_values');
    expect(saved['ui.style']).toBe('junior');
    expect(Number(saved['voice.tts_speed'])).toBeLessThanOrEqual(85);
    expect(saved['notify.earcons']).toBe(true);
    expect(saved['ui.simple']).toBe(true);
    await expectAccessible(alfa, 'K.3 Wygląd w stylu Junior');

    // Uproszczony ekran w rozmowie: mniej przycisków, wskaźniki i tryb pracy zostają.
    await main.keyboard.press('Control+,');
    await expect(main.locator('#alfa-composer')).toBeVisible();
    for (const name of ['Pokaż panel Sesje', 'Ukryj panel Sesje', 'Pokaż panel prawy']) {
      await expect(main.getByRole('button', { name, exact: true })).toHaveCount(0);
    }
    await expect(main.locator('.edge-tab')).toHaveCount(0);
    const composer = main.locator('form.composer');
    await expect(composer.getByRole('button', { name: /^Adresatka:/ })).toHaveCount(0);
    await expect(composer.getByRole('button', { name: /^Tryb pracy: / })).toBeVisible();
    // Wskaźnik mikrofonu: „Mikrofon: …”, a bez składników głosu (instancja testowa) „Głos niedostępny”.
    await expect(
      main
        .locator('header.titlebar')
        .getByRole('button', { name: /^(Mikrofon:|Głos niedostępny)/ }),
    ).toBeVisible();
    await expect(main.getByRole('button', { name: 'Ustawienia (Ctrl+,)' })).toBeVisible();
    await shot(alfa, 'k3b-junior-uproszczony');
    await expectAccessible(alfa, 'K.3 rozmowa: Junior, uproszczony ekran');
  } finally {
    // Wygląd przywracany tą samą drogą co użytkownik (okno trzyma ustawienia w pamięci i nie
    // śledzi zmian zrobionych z zewnątrz); głos i dźwięki — przez rdzeń.
    const style = typeof before['ui.style'] === 'string' ? before['ui.style'] : 'classic';
    await main.keyboard.press('Escape');
    await main.keyboard.press('Control+,');
    await main
      .getByRole('navigation', { name: 'Sekcje ustawień' })
      .getByRole('button', { name: 'Wygląd' })
      .click();
    await pickOption(main.getByRole('combobox', { name: 'Styl', exact: true }), style);
    const simple = main.getByRole('switch', { name: 'Uproszczony ekran' });
    if ((await simple.isChecked()) !== (before['ui.simple'] === true)) await simple.click();
    await main.keyboard.press('Control+,');
    for (const key of ['voice.tts_speed', 'notify.earcons'] as const) {
      const value = before[key];
      if (value === undefined) await call(main, 'settings_reset', { key });
      else await call(main, 'settings_set', { key, value });
    }
  }
  const after = await call<Record<string, unknown>>(main, 'settings_values');
  for (const key of KEYS) expect(after[key], key).toEqual(before[key]);
  await expect(main.locator('#alfa-composer')).toBeVisible();
});
