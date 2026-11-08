# cost-meter-contract

Kontrakt licznika kosztów (docs/modules/cost-meter/SPEC.md, PLAN §14.6, §5.5). Wszystkie kwoty to
liczby całkowite: mikro-USD, mikro-PLN (1 PLN = 10⁶), kurs ×10⁴; iloczyny w `u128`, nasycenie.

- `Usage`, `cost_micro_usd`, `usd_to_pln`, `format_pln`; `CostInput` (`Pricing::Price | Reported | Unknown`
  — brak ceny to koszt „nieznany”, nie 0), `CostRecord` (z kursem użytym do przeliczenia), `estimate`.
- `Ledger` — agregaty przyrostowe z rekordów: całość, sesja, dzień, miesiąc, dostawca×miesiąc, tło.
- Kurs: `FxSource` (port), `parse_nbp_json` (NBP tabela A, bez `f64`), `FxCache` (raz dziennie,
  poprzedni kurs oznaczony `stale`, bez notowania — kurs zapasowy), `NBP_USD_URL`.
- Budżet: `BudgetConfig` (limit miesięczny `Enforced | AlertOnly | Off`, budżet tła — domyślnie 0 PLN,
  limity dostawców z `accounts-hub`, `warn_at_pct`, progi alertów 50/80/100, kurs zapasowy),
  `evaluate` → `Allow | Warn | Block` (Block tylko przy włączonym limicie), `crossed_thresholds`,
  `BudgetOrigin` (Ulepszacz/agentki nie zmieniają limitów).
- Trait `CostMeter`, `LedgerStore`, `CostClock`, zdarzenia `cost.recorded`, `cost.limit.warning|blocked`,
  `cost.fx.updated|stale`. Feature `contract-tests`: `contract_tests::run_all`.
