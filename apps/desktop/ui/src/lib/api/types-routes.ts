// „Gdzie trafiają dane” (komenda `compliance_routes`): trasy z rejestru zgodności, tylko odczyt.

export interface DataRoute {
  readonly id: string;
  readonly provider: string;
  /** `api`, `cli-p` albo `sdk`. */
  readonly mode: string;
  /** Kody jurysdykcji; pusta lista — nieznana. */
  readonly jurisdiction: readonly string[];
  /** Tagi prywatności (`eu`, `cn-may-train`, `unknown`…). */
  readonly privacy: readonly string[];
  /** `no_train`, `may_train`, `unknown`. */
  readonly training: string;
  /** `green`, `grey`, `forbidden`. */
  readonly status: string;
  readonly stale: boolean;
  readonly unverified: boolean;
  readonly enabled: boolean;
  readonly standard_allowed: boolean;
  readonly private_allowed: boolean;
  /** Powód decyzji dla sesji prywatnej (po polsku). */
  readonly private_reason: string;
}

export interface DataRoutesView {
  readonly routes: readonly DataRoute[];
}
