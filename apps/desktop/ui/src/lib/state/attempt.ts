// Jedna obsługa błędów akcji i wczytywania w widokach: komenda rdzenia, która się nie uda, ma dać
// widoczny komunikat (toast albo stan „nie udało się wczytać" z „Ponów"), nigdy ciszę. Test na
// laptopie (2026-10-07): odrzucona komenda bez obsługi = przycisk „nic nie robi".
import { errorText } from '../api/command-error';
import type { ToastState } from './toasts.svelte';

/** Toast z komunikatem błędu (komunikat rdzenia albo tekst wyjątku). */
export function showError(toasts: ToastState, error: unknown): void {
  toasts.show({ kind: 'error', message: errorText(error) });
}

/**
 * Wykonuje akcję; przy błędzie pokazuje toast. Zwraca `true` po sukcesie: pola czyści się tylko
 * wtedy, a po `false` wołający cofa zmiany optymistyczne.
 */
export async function attempt(
  toasts: ToastState,
  action: () => Promise<unknown>,
): Promise<boolean> {
  try {
    await action();
    return true;
  } catch (error) {
    showError(toasts, error);
    return false;
  }
}

/** Stan wczytywania widoku: dane, błąd do pokazania (z przyciskiem „Ponów") albo trwa. */
export type Loadable<T> =
  | { readonly status: 'loading' }
  | { readonly status: 'ready'; readonly value: T }
  | { readonly status: 'failed'; readonly error: string };

/** Wczytuje dane do `Loadable` — błąd nie zostawia widoku pustego ani w „Ładowanie…" na zawsze. */
export async function load<T>(fetch: () => Promise<T>): Promise<Loadable<T>> {
  try {
    return { status: 'ready', value: await fetch() };
  } catch (error) {
    return { status: 'failed', error: errorText(error) };
  }
}
