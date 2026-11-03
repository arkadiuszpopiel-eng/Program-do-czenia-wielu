// Interfejsy Brokera: stan (`broker_status`) i uprawnienia `permissions_*` (części `AlfaClient`).
import type { AutonomyLevel } from './types';
import type { BrokerStatusView } from './types-broker';
import type {
  BrokerIntentResult,
  ChildLockRequest,
  ChildLockState,
  GrantView,
  MaxAccess,
  PermissionsState,
} from './types-hub';

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

/** Uprawnienia: poziomy autonomii, „Max dostęp” i zgody — zmiany wyłącznie przez Sędziego. */
export interface PermissionsApi extends GrantsApi {
  get(sessionId: string | null): Promise<PermissionsState>;
  /** Intencja: zmiana poziomu autonomii — potwierdzenie wyłącznie w oknie Brokera. */
  requestLevel(level: AutonomyLevel, sessionId: string | null): Promise<BrokerIntentResult>;
  /** Intencja: przenosi do okna Brokera z tą prośbą o zatwierdzenie. */
  openApproval(approvalId: string): Promise<BrokerIntentResult>;
  /**
   * Intencja: „Max dostęp” (plan 50 §8.1) — włączenie zasięgu potwierdzasz w oknie Sędziego
   * Cieni; `null` wyłącza od razu.
   */
  requestMaxAccess(scope: MaxAccess | null): Promise<BrokerIntentResult>;
  /**
   * Profil Dziecko (plan 50 §4.4): `null` — odczyt; zmiana z PIN-em rodzica, który sprawdza
   * wyłącznie Sędzia Cieni (zły PIN — `invalid_input`).
   */
  childLock(change: ChildLockRequest | null): Promise<ChildLockState>;
}
