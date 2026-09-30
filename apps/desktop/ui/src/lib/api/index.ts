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
  return new FakeAlfaClient({ scenario });
}
