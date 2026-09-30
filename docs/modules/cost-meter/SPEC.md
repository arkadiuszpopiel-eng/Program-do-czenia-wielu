# cost-meter — SPEC (szkic v0)

## Cel
Liczenie kosztów i limitów: koszt per wywołanie (USD z tabeli cen) → PLN kursem NBP (tabela A, raz dziennie, kurs zapasowy), koszt sesji/dnia/miesiąca, limit miesięczny w PLN (wyłączalny), osobny budżet tła, alerty, szacunek przed długim zadaniem, wskaźnik okna planu (best effort) (PLAN §14.6, §5.5).

## Fala i priorytet
F1. P0. Okna planów mostów — F4.

## Kontrakt (szkic Rust)
```rust
// cost-meter-contract — SZKIC
pub struct Money { pub minor: i64, pub currency: Currency /* USD | PLN */ }
pub struct Usage { pub input_tokens: u64, pub output_tokens: u64, pub cached_tokens: u64, pub audio_seconds: f32, pub chars_tts: u64 }
pub struct CostRecord { pub ts: Timestamp, pub session: SessionId, pub agent: Option<PersonaId>, pub account: AccountId,
                        pub model: ModelId, pub usage: Usage, pub usd: Money, pub pln: Money, pub rate: FxRate, pub background: bool }
pub trait CostMeter: Send + Sync {
    fn record(&self, r: CostRecord) -> Result<()>;
    fn totals(&self, q: TotalsQuery /* Session | Day | Month | Account */) -> Totals;
    fn check(&self, est: Money, background: bool) -> BudgetVerdict /* Ok | Warn { remaining } | Blocked { limit } */;
    fn estimate(&self, plan: &TaskEstimateInput) -> Money;
    fn set_limit(&self, limit: Option<Money> /* None = wyłączony */) -> Result<()>;
}
```
Zdarzenia: `cost.recorded`, `cost.limit.warning` (80%, 100%), `cost.limit.blocked`, `cost.fx.updated`, `cost.fx.stale` (kurs zapasowy), `cost.plan_window.estimate`.

## Zależności
`core-bus/config/log-contract`, `accounts-hub-contract` (tabele cen z katalogu, limity per dostawca), `platform-windows-contract`/`net` (pobranie kursu NBP — przez `net.egress(api.nbp.pl)`).

## Niezmienniki
- Ceny wyłącznie z konfiguracji/katalogu (PLAN §5.5); brak cen w kodzie; brak ceny = koszt „nieznany" oznaczony w UI, nie 0.
- Limit miesięczny wyłączalny; po wyłączeniu zostaje wskaźnik zużycia i opcjonalne alerty — nic nie blokuje.
- Budżet tła domyślnie obejmuje tylko modele lokalne; zadania tła na API wymagają jawnego budżetu.
- Zapis kosztu jest częścią strumienia ModelCalls (`core-log`); `totals` liczone z rekordów, nie z licznika w pamięci (odtwarzalne po restarcie).
- Limity budżetów należą do polityk Jądra: `improver` ich nie zmienia.
- Brak kursu online → kurs zapasowy z konfiguracji + zdarzenie `fx.stale`; nigdy blokada z powodu braku kursu.

## Zdolności / uprawnienia
`net.egress(api.nbp.pl)` raz dziennie.

## Izolacja
`inproc`, `always` (deterministyczna usługa systemowa, PLAN §9.2).

## Budżet zasobów
RAM ≤ 2 MB; `record` ≤ 0,5 ms; `totals` z cache przyrostowego ≤ 5 ms.

## Konfiguracja (klucze TOML)
`[cost] monthly_limit_pln = 100` (albo `false`), `warn_at_pct = 80`, `background_budget_pln = 0`, `fx.source = "nbp_a"`, `fx.fallback_usd_pln = 4.0`, `fx.refresh = "24h"`; `[accounts.<id>] cost_limit_pln`.

## Wkład do UI
Koszt sesji w pasku górnym (klik = szczegóły), licznik kontekstu i kosztu, pulpit kosztów/limitów, szacunek przed długim zadaniem, ostrzeżenia, Ustawienia → Modele i dostawcy (limity).

## Testy akceptacyjne
- `ACC-F1-cost-meter-01`: property-based — sumy per sesja/dzień/miesiąc równe sumie rekordów; przeliczenie PLN zgodne z kursem z rekordu.
- `ACC-F1-cost-meter-02`: limit włączony → `Blocked` po przekroczeniu; limit wyłączony → nigdy `Blocked`.
- `ACC-F1-cost-meter-03`: brak sieci → kurs zapasowy, zdarzenie `fx.stale`, brak blokady.

## Fake
`cost-meter-fake`: rekordy w pamięci, kurs stały, sterowany werdykt `check()` dla testów `agent-runtime`/`router`.

## Otwarte pytania
- Rozliczanie audio (STT sekundy, TTS znaki) w jednym `Usage` czy per rodzaj — do ustalenia w SPEC v1 (F2).
- Estymata okna planu mostów (Claude Code/Codex) — F4.
