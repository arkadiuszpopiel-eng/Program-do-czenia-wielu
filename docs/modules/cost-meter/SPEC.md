# cost-meter — SPEC (v0)

## Cel
Liczenie kosztów i limitów: koszt per wywołanie (USD z tabeli cen) → PLN kursem NBP (tabela A, raz dziennie, kurs zapasowy), koszt sesji/dnia/miesiąca/dostawcy, limit miesięczny w PLN (wyłączalny), osobny budżet tła, alerty, szacunek przed długim zadaniem (PLAN §14.6, §5.5). Wskaźnik okna planu mostów — F4.

## Fala i priorytet
F1. P0. v0 **zrobione** (bez okien planów i rozliczania audio).

## Kontrakt (`cost-meter-contract` — źródło prawdy)
- Kwoty wyłącznie całkowite: mikro-USD, mikro-PLN (1 PLN = 10⁶), kurs ×10⁴ (3,6512 → 36 512); iloczyny w `u128`, zaokrąglenie połowa w górę, nasycenie.
- `CostInput { session, agent, provider, account, model, usage: Usage { input, output, cache_read, cache_write }, pricing: Price(ModelPrice) | Reported { micro_usd } | Unknown, background }`.
- `CostRecord { seq, ts, day (lokalny), …, micro_usd: Option, micro_pln: Option, fx: FxRate, background }` — koszt nieznany = `None`, nie 0.
- `Ledger` — agregaty przyrostowe z rekordów; `TotalsQuery = All | Session | Day | Month | Provider { provider, month } | Background { month }`.
- `BudgetConfig { monthly: MonthlyLimit { amount_micro_pln, mode: Enforced | AlertOnly | Off }, background (domyślnie 0 PLN, Enforced), providers: ProviderId → CostLimit, warn_at_pct = 80, alert_thresholds_pct = [50, 80, 100], fallback_rate_e4 = 40 000 }`.
- `evaluate` → `BudgetDecision = Allow | Warn { notices } | Block { notice }`; Block tylko gdy dany limit `Enforced` i `wydano + szacunek > limit`; kolejność: miesiąc → tło → dostawca.
- Kurs: `FxSource` (port), `parse_nbp_json` (bez `f64`, kontrola zakresu 0,5–50), `FxCache` (raz dziennie; poprzedni kurs `stale`; bez notowania — zapasowy).
- `trait CostMeter { record, totals, estimate, check_budget, budget, set_budget(config, BudgetOrigin), current_rate, refresh_fx }`, `LedgerStore`, `CostClock`.
Zdarzenia: `cost.recorded`, `cost.limit.warning` (każdy próg raz, per zakres), `cost.limit.blocked`, `cost.fx.updated`, `cost.fx.stale`.

## Zależności
`accounts-hub-contract` (`ProviderId`, `ModelPrice`, `CostLimit`), `core-bus-contract`, `core-registry-contract`. Kurs NBP przez port `HttpGet` (kompozycja; `net.egress(api.nbp.pl)`).

## Niezmienniki
- Ceny wyłącznie z konfiguracji/katalogu; brak ceny = koszt „nieznany", liczony osobno (`unknown_cost_calls`).
- Suma agregatów (sesje, dni, miesiące, dostawcy) = suma wpisów; PLN rekordu = USD × kurs z rekordu (property-based).
- Limit wyłączony (`AlertOnly`/`Off`) nigdy nie blokuje; brak kursu nigdy nie blokuje (kurs zapasowy + `cost.fx.stale`).
- Budżet tła domyślnie 0 PLN (tylko modele lokalne); płatne zadania tła wymagają jawnego budżetu.
- `totals` liczone z rekordów (dziennik odtwarzany po restarcie, kurs z ostatniego rekordu NBP).
- Limity są polityką Jądra: `set_budget` tylko `User`/`Broker` (Ulepszacz i agentki → `NotPermitted`).

## Zdolności / uprawnienia
`net.egress(api.nbp.pl)` raz dziennie.

## Izolacja
`inproc`, `always`; start bez sieci — gospodarz wywołuje `refresh_fx()` w tle po starcie i codziennie.

## Budżet zasobów
RAM ≤ 2 MB; `record` ≤ 0,5 ms (bez sieci; dopisanie linii NDJSON); `totals` z cache przyrostowego ≤ 5 ms.

## Trwałość
Dziennik **NDJSON** (`NdjsonLedger`, append-only, jedna linia na rekord), nie SQLite: koszty nie są tajne, format zgodny ze strumieniem ModelCalls, czytelny, bez budowania SQLite/OpenSSL; urwana linia po awarii pomijana i domykana. Rotacja miesięczna pliku — SPEC v1.

## Konfiguracja (klucze TOML)
`[cost] monthly_limit_pln = 100`, `monthly_limit_mode = "enforced" | "alert_only" | "off"`, `warn_at_pct = 80`, `alert_thresholds_pct = [50, 80, 100]`, `background_budget_pln = 0`, `fx.source = "nbp_a"`, `fx.fallback_usd_pln = 4.0`, `fx.refresh = "24h"`; `[accounts.<id>] cost_limit` (z `accounts-hub`).

## Wkład do UI
Koszt sesji w pasku górnym, licznik kontekstu i kosztu, pulpit kosztów/limitów (`format_pln`), szacunek przed długim zadaniem, ostrzeżenia, Ustawienia → Modele i dostawcy (limity).

## Testy akceptacyjne
- `ACC-F1-cost-meter-01`: property-based — sumy per sesja/dzień/miesiąc/dostawca równe sumie rekordów; PLN zgodne z kursem z rekordu.
- `ACC-F1-cost-meter-02`: limit włączony → `Block` po przekroczeniu; wyłączony → nigdy `Block` (property-based).
- `ACC-F1-cost-meter-03`: brak sieci → kurs zapasowy, `cost.fx.stale`, brak blokady.

## Fake
`cost-meter-fake`: `FakeCostMeter` (rekordy w pamięci, kurs stały, `force_decision`, rejestr `checks`), `MemoryLedgerStore`, `FakeFxSource`, `FixedClock`.

## Otwarte pytania
- Rozliczanie audio (STT sekundy, TTS znaki) — SPEC v1 (F2).
- Estymata okna planu mostów (Claude Code/Codex) — F4.
- Zapis w strumieniu ModelCalls `core-log` zamiast osobnego pliku — po `core-log-impl`.
