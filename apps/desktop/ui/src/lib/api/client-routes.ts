// Interfejs widoku „Gdzie trafiają dane” (część `AlfaClient`; komenda `compliance_routes`).
import type { DataRoutesView } from './types-routes';

export interface DataRoutesApi {
  /** Trasy do dostawców z jurysdykcją, tagami prywatności i decyzją dla sesji prywatnej. */
  list(): Promise<DataRoutesView>;
}
