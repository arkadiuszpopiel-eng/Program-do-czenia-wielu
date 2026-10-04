// Logika widoku Brokera (bez Svelte — testowana w vitest): który baner pokazać w oknie głównym
// i czy aplikacja jest w bezpiecznym stanie (wszystko, co wymaga zgody, jest odrzucane).
import type { BrokerStatusView } from '../api/types-broker';

/**
 * Baner Brokera: `unavailable`/`lost` — bezpieczny stan (alert, bez ukrywania), `connecting` —
 * łączenie, `watchdog` — kill-switch awaryjnie w aplikacji, `dev` — Broker w procesie.
 */
export type BrokerBanner = 'unavailable' | 'lost' | 'connecting' | 'watchdog' | 'dev';

/** Bezpieczny stan: brak Brokera albo łącze niedziałające. */
export function safeState(view: BrokerStatusView | null): boolean {
  return view !== null && (view.mode === 'unavailable' || view.state !== 'connected');
}

/** Najważniejszy baner (albo brak); `dismissed` — banery ukryte przez użytkownika. */
export function brokerBanner(
  view: BrokerStatusView | null,
  dismissed: readonly BrokerBanner[] = [],
): BrokerBanner | null {
  if (view === null) return null;
  const pick = (b: BrokerBanner): BrokerBanner | null => (dismissed.includes(b) ? null : b);
  if (view.mode === 'unavailable') return 'unavailable';
  if (view.state === 'lost') return 'lost';
  if (view.state === 'connecting') return 'connecting';
  if (view.mode === 'in_process') return pick('dev');
  if (!view.watchdog) return pick('watchdog');
  return null;
}

/** Banery, których nie da się ukryć (bezpieczny stan). */
export function bannerIsSticky(banner: BrokerBanner): boolean {
  return banner === 'unavailable' || banner === 'lost';
}
