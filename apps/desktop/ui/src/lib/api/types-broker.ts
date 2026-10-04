// Stan Brokera (komenda `broker_status`, zdarzenie `BrokerStatus`) — kształt 1:1
// z `crates/app-api/src/dto/broker.rs` (pola snake_case). UI tylko pokazuje stan i wyjaśnienie;
// zatwierdzanie odbywa się wyłącznie w oknie Brokera (PLAN §8.2), nigdy w WebView.
import type { LocalizedText } from './types';

/**
 * Gdzie działa Broker: `service` — usługa Windows na osobnym koncie (pełna izolacja),
 * `portable` — proces potomny Alfy na koncie użytkownika (słabsza izolacja), `in_process` —
 * w procesie aplikacji (tryb deweloperski, bez okna zatwierdzeń), `unavailable` — brak Brokera.
 */
export type BrokerMode = 'service' | 'portable' | 'in_process' | 'unavailable';

/** Łącze z Brokerem; `lost` = bezpieczny stan: wszystko, co wymaga zgody, jest odrzucane. */
export type BrokerLinkState = 'connected' | 'connecting' | 'lost';

export interface BrokerStatusView {
  readonly mode: BrokerMode;
  readonly state: BrokerLinkState;
  /** Działa okno zatwierdzeń (Broker-UI). */
  readonly approval_window: boolean;
  /** `Ctrl+Shift+F12` obsługuje `alfa-watchdog` poza UI (inaczej awaryjnie aplikacja). */
  readonly watchdog: boolean;
  /** Broker na osobnym koncie (usługa). */
  readonly isolated: boolean;
  /** Wyjaśnienie trybu albo przyczyny zerwania (zwykły tekst). */
  readonly detail: LocalizedText | null;
}
