// Wybór implementacji: w oknie Tauri — IPC; w przeglądarce (dev, Storybook, testy E2E) — atrapa.
// Atrapa jest ładowana dynamicznie, więc nie wchodzi do paczki startowej buildu produkcyjnego.
import type { AlfaClient } from './client';
import { TauriAlfaClient } from './tauri-client';

export type { AlfaClient } from './client';

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** Scenariusz atrapy z adresu, np. `?scenario=offline` (dev i testy E2E). */
export function scenarioFromUrl(): string | null {
  if (typeof location === 'undefined') return null;
  return new URLSearchParams(location.search).get('scenario');
}

export async function createClient(): Promise<AlfaClient> {
  if (isTauri()) return new TauriAlfaClient();
  const { FakeAlfaClient, FAKE_SCENARIOS } = await import('./fake/fake-client');
  const wanted = scenarioFromUrl();
  const scenario = FAKE_SCENARIOS.find((s) => s === wanted) ?? 'default';
  // Podgląd wyglądu bez klikania w ustawieniach: `?style=glass|next&theme=light|dark`.
  const params = new URLSearchParams(location.search);
  const settings: Record<string, string | boolean> = {};
  const style = params.get('style');
  const theme = params.get('theme');
  const locale = params.get('locale');
  if (style) settings['ui.style'] = style;
  if (theme) settings['ui.theme'] = theme;
  if (locale === 'en' || locale === 'pl') settings['ui.locale'] = locale;
  const avatars = params.get('avatars');
  if (avatars) settings['ui.avatars'] = avatars;
  // Panele: `?panels=autohide|docked`. Domyślnie chowane (właściciel 29.10), ale testy automatyczne
  // (navigator.webdriver) startują z zadokowanym układem, na którym opiera się większość scenariuszy —
  // chowanie sprawdzają osobno z `?panels=autohide`.
  const panels = params.get('panels') ?? (navigator.webdriver ? 'docked' : null);
  if (panels) settings['ui.panels_autohide'] = panels === 'autohide';
  // Włączanie mikrofonu (Spacja PTT): `?mic=ptt`.
  if (params.get('mic') === 'ptt') settings['voice.mode'] = 'ptt';
  // Uproszczony ekran (K.3): `?simple=1`.
  if (params.get('simple') === '1') settings['ui.simple'] = true;
  // Podgląd stanów obsady: `?cast=speaking,working,waiting_approval,idle`.
  const cast = params.get('cast')?.split(',') ?? [];
  return new FakeAlfaClient({ scenario, settings, cast });
}
