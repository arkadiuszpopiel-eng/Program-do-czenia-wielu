// Silnik gier NextGO.Gray na PRAWDZIWYM rdzeniu: pole „Katalog silnika gier NextGO.Gray” w Ustawienia →
// Zaawansowane pochodzi ze schematu rdzenia, ścieżka wpisana w interfejsie trafia do konfiguracji
// (ten sam klucz, który śledzą narzędzia `engine_*` — `app_agents::engine::watch_root`), a
// przywrócenie domyślnej wartości ją usuwa.
import { expect, test } from './alfa';
import { call } from './ipc-call';

const KEY = 'engine.nextgo.root';

test('Ustawienia → Zaawansowane: katalog silnika gier zapisany w rdzeniu', async ({ alfa }) => {
  const { main } = alfa;
  const engine = process.env['ALFA_NEXTGO_ROOT'] ?? 'C:\\Gry\\NextGO.Gray';
  const nav = main.getByRole('navigation', { name: 'Sekcje ustawień' });
  if (!(await nav.isVisible())) await main.keyboard.press('Control+,');
  await nav.getByRole('button', { name: 'Zaawansowane' }).click();
  const field = main.getByRole('textbox', { name: 'Katalog silnika gier NextGO.Gray' });
  await expect(field).toHaveValue('');
  await field.fill(engine);
  await field.press('Tab');
  await expect
    .poll(async () => (await call<Record<string, unknown>>(main, 'settings_values'))[KEY])
    .toBe(engine);
  await main
    .getByRole('button', { name: 'Przywróć domyślne: Katalog silnika gier NextGO.Gray' })
    .click();
  await expect(field).toHaveValue('');
  await expect
    .poll(async () => (await call<Record<string, unknown>>(main, 'settings_values'))[KEY] ?? '')
    .toBe('');
  await main.keyboard.press('Escape');
});
