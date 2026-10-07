// Interfejs stanu Brokera (część `AlfaClient`; komenda `broker_status` w COMMANDS.md).
import type { BrokerStatusView } from './types-broker';

/** Broker: tylko odczyt stanu — zmiany i zatwierdzenia wyłącznie w oknie Brokera. */
export interface BrokerApi {
  /** Tryb (usługa / przenośny / w procesie / brak), łącze, okno zatwierdzeń, watchdog. */
  status(): Promise<BrokerStatusView>;
}
