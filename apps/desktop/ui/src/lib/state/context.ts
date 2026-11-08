// Kontekst Svelte: jedna instancja AppState na okno, dostępna w każdym komponencie.
import { getContext, setContext } from 'svelte';
import type { AppState } from './app.svelte';

const KEY = Symbol('alfa-app');

export function provideApp(app: AppState): AppState {
  return setContext(KEY, app);
}

export function useApp(): AppState {
  const app = getContext<AppState | undefined>(KEY);
  if (!app) throw new Error('AppState nie jest dostępny w kontekście komponentu');
  return app;
}
