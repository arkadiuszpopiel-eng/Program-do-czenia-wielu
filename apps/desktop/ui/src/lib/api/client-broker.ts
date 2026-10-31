// Interfejs stanu Brokera (część `AlfaClient`; komenda `broker_status` w COMMANDS.md).
import type { BrokerStatusView } from './types-broker';
import type { GrantView } from './types-hub';

/** Broker: tylko odczyt stanu — zmiany i zatwierdzenia wyłącznie w oknie Brokera. */
export interface BrokerApi {
  /** Tryb (usługa / przenośny / w procesie / brak), łącze, okno zatwierdzeń, watchdog. */
  status(): Promise<BrokerStatusView>;
}

/** Zgody „zawsze w tym zakresie” (część `PermissionsApi`; nadawanie — wyłącznie okno Sędziego). */
export interface GrantsApi {
  /** Zgody z Sędziego Cieni (czas wybrany w oknie albo do odwołania). */
  grants(): Promise<readonly GrantView[]>;
  /** Odwołanie zgody — tylko zawęża uprawnienia, bez okna Sędziego; `false` — już jej nie ma. */
  revokeGrant(grantId: string): Promise<boolean>;
}
