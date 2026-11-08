// katalog-przeglad: ustawienie
// Przegląd ogólny katalogu funkcji (docs/plan/45-testy-ai.md §4) na PRAWDZIWYM rdzeniu: KAŻDE
// ustawienie ze schematu — także dodane po napisaniu tego testu — dostaje inną poprawną wartość
// (przełącznik odwrotny, inna opcja listy, liczba o krok w zakresie, tekst bez zmiany), rdzeń ma
// ją zapamiętać, potem wraca poprzednia wartość. Profil instancji testowej jest tymczasowy.
// Pominięte ze skutkiem poza programem: autostart (wpis w rejestrze konta Windows — wspólnym
// z Alfą właściciela) i kamera (włączenie urządzenia). Ich działanie sprawdzają testy rdzenia.
import { expect, test } from './alfa';
import { call } from './ipc-call';

type Value = boolean | number | string;
interface Control {
  kind: 'toggle' | 'select' | 'number' | 'text';
  options?: { value: string }[];
  min?: number;
  max?: number;
  step?: number;
}
interface SettingDef {
  key: string;
  control: Control;
  default: Value;
}
interface PageDef {
  id: string;
  settings: SettingDef[];
}

const SKIP: Record<string, string> = {
  'general.start_with_system': 'autostart w rejestrze konta Windows',
  'camera.enabled': 'włącza kamerę',
};

/** Inna poprawna wartość ustawienia (tekst: ta sama — sam zapis i odczyt). */
function other(def: SettingDef, current: Value): Value {
  const c = def.control;
  if (c.kind === 'toggle') return !current;
  if (c.kind === 'select') {
    return c.options?.find((o) => o.value !== current)?.value ?? current;
  }
  if (c.kind === 'number') {
    const step = c.step && c.step > 0 ? c.step : 1;
    const n = Number(current);
    const up = n + step;
    return up <= (c.max ?? up) ? up : Math.max(c.min ?? n - step, n - step);
  }
  return current;
}

test('każde ustawienie: zapis innej wartości, odczyt i przywrócenie na prawdziwym rdzeniu', async ({
  alfa,
}) => {
  test.setTimeout(240_000);
  const { main } = alfa;
  const pages = await call<PageDef[]>(main, 'settings_schema');
  const defs = pages.flatMap((p) => p.settings);
  expect(defs.length, 'ustawienia w schemacie').toBeGreaterThan(50);
  const before = await call<Record<string, Value>>(main, 'settings_values');
  const failed: string[] = [];
  let checked = 0;
  for (const def of defs) {
    if (SKIP[def.key]) continue;
    const current = before[def.key] ?? def.default;
    const next = other(def, current);
    try {
      await call(main, 'settings_set', { key: def.key, value: next });
      const after = await call<Record<string, Value>>(main, 'settings_values');
      if ((after[def.key] ?? def.default) !== next) {
        failed.push(`${def.key}: zapisane ${String(next)}, odczytane ${String(after[def.key])}`);
      }
      await call(main, 'settings_set', { key: def.key, value: current });
      const restored = await call<Record<string, Value>>(main, 'settings_values');
      if ((restored[def.key] ?? def.default) !== current) {
        failed.push(`${def.key}: nie wróciło do ${String(current)}`);
      }
      checked++;
    } catch (e) {
      failed.push(`${def.key}: ${(e as Error).message.split('\n')[0]}`);
    }
  }
  expect(failed, 'ustawienia z błędem').toEqual([]);
  expect(checked).toBe(
    defs.length - Object.keys(SKIP).filter((k) => defs.some((d) => d.key === k)).length,
  );
});
