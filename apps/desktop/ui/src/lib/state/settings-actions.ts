// Zmiany ustawień i skrótów z obsługą błędów (wydzielone z AppState). Wartość działa od razu
// (wygląd, język, skrót); gdy rdzeń ją odrzuci (np. skrót Ctrl+Alt+litera kolidujący z polskim
// AltGr), wraca poprzednia i jest toast z powodem — widok nigdy nie pokazuje wartości, której nie
// zapisano. Wynik `true` tylko po sukcesie.
import type { SettingValue } from '../api/types-system';
import type { AppState } from './app.svelte';
import { attempt } from './attempt';

const isLocale = (value: unknown): value is 'pl' | 'en' => value === 'pl' || value === 'en';

export async function setSetting(
  app: AppState,
  key: string,
  value: SettingValue,
): Promise<boolean> {
  const before = app.settings[key];
  const localeBefore = app.i18n.locale;
  app.settings[key] = value;
  const ok = await attempt(app.toasts, async () => {
    if (key === 'ui.locale' && isLocale(value)) await app.i18n.setLocale(value);
    await app.client.settings.set(key, value);
  });
  // Cofnięcie tylko, gdy w międzyczasie nie przyszła nowsza zmiana (np. dwa kroki powiększenia).
  if (!ok && app.settings[key] === value) {
    if (before === undefined) delete app.settings[key];
    else app.settings[key] = before;
    if (app.i18n.locale !== localeBefore) await app.i18n.setLocale(localeBefore);
  }
  return ok;
}

/** Przywrócenie domyślnej — wartość z rdzenia dopiero po sukcesie (bez zmiany optymistycznej). */
export function resetSetting(app: AppState, key: string): Promise<boolean> {
  return attempt(app.toasts, async () => {
    const value = await app.client.settings.reset(key);
    app.settings[key] = value;
    if (key === 'ui.locale' && isLocale(value)) await app.i18n.setLocale(value);
  });
}

/** Skrót (`null` — domyślny, `''` — wyłączony); odrzucony przez rdzeń wraca do poprzedniego. */
export async function setShortcut(
  app: AppState,
  actionId: string,
  chord: string | null,
): Promise<boolean> {
  const before = app.shortcutOverrides[actionId];
  const apply = (next: string | null | undefined): void => {
    if (next === null || next === undefined) delete app.shortcutOverrides[actionId];
    else app.shortcutOverrides[actionId] = next;
  };
  apply(chord);
  const ok = await attempt(app.toasts, () => app.client.settings.setShortcut(actionId, chord));
  if (!ok && app.shortcutOverrides[actionId] === (chord ?? undefined)) apply(before);
  return ok;
}
