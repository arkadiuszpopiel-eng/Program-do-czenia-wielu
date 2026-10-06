# evals — SPEC (v1: kontrakt zaimplementowany)

## Cel
Harness ewaluacji (PLAN §4.4, §12.4; ACCEPTANCE §1, §13): wspólny format zestawów (manifest, wersja, SHA-256 zamrożenia, podział dev/test/**holdout**, progi), weryfikacja integralności (zmiana zestawu zamrożonego = błąd), runner metryk (agregacja per przypadek i klasa, przedziały ufności bootstrap, porównanie wariantów), raporty JSON/Markdown i **bramka ewaluacyjna Jądra** z ukrytym holdoutem, niedostępnym dla Ulepszacza. Nie uruchamia systemu sam — wariant w piaskownicy uruchamia port Jądra.

## Fala i priorytet
F8 (rdzeń formatu od F0 przez manifesty F2/F5/F7). P1.

## Kontrakt (źródło prawdy: `crates/evals-contract`)
```rust
pub struct SuiteManifest { schema, suite: SuiteId, wave, version, status: Proposed|Frozen|Retired, created, accepted_by, files: BTreeMap<ścieżka, sha256>, cases: Vec<CaseSource{path, format, split?}>, thresholds: Vec<Threshold{id, rule: Metric{metric, op: Ge|Le, value, per_class, point}|Text}> }
pub enum Split { Dev, Test, Holdout }   pub enum CaseFormat { EvalCases, F2VoiceManifest, F3ToolTasks, F7RecallQueries, Opaque }
pub struct EvalCase { id, split, class?, input, expected }   pub struct CaseOutcome { case_id, class?, repeat, passed, metrics }
pub trait SuiteCatalog { fn suites(); fn manifest(&SuiteId); fn verify(&SuiteId) -> IntegrityReport; fn cases(&SuiteId, Split) /* Holdout → HoldoutSealed */ }
#[async_trait] pub trait CandidateRunner { async fn run(&Variant, &EvalCase, repeat) -> Result<CaseOutcome, String> }   // Jądro (app-*), nigdy Ulepszacz
#[async_trait] pub trait EvalGate { async fn evaluate(GateRequest{suite, stage: Sandbox|Holdout, baseline, candidate, repeats, primary_metric?}) -> GateVerdict; fn policy() -> GatePolicy }
pub fn aggregate/check_thresholds/compare/bootstrap_mean/bootstrap_paired_delta; pub fn decide/evaluate_cases/run_variant; QueryBudget; build_report → EvalReport{to_json, to_markdown}
```
Zdarzenia: `evals.run.completed`, `evals.gate.decided` (werdykt zbiorczy), `evals.integrity.failed` (liczby, bez ścieżek holdoutu).

## Zależności
`core-bus-contract`, `core-registry-contract` (impl). Zewnętrzne: `sha2` (już w `Cargo.lock`).

## Niezmienniki
- Statystyka deterministyczna (SplitMix64, ziarno w `BootstrapConfig`); jednostką próby jest przypadek (powtórzenia uśrednione); progi na granicy przedziału (`≥` dolna, `≤` górna), `per_class` — każda klasa osobno.
- Zestaw `frozen` ze zmienionym plikiem nie wydaje przypadków (`IntegrityViolation`); holdout sprawdzany **ściśle przy każdej ocenie**.
- Katalog publiczny nie czyta `holdout/` ani `corpus/` (pierwszy segment bez rozróżniania wielkości liter, kanonizacja — dowiązania poza korzeń odrzucone, ścieżki bez `..`, `\`, `:`); manifest publiczny ze źródłem holdoutu lub plikiem w katalogu zapieczętowanym jest odrzucany.
- Werdykt holdoutu: tylko liczby (zaokrąglone do `report_decimals`), bez identyfikatorów, treści i nazw klas; `GatePolicy` nie słabsza niż plan (N ≥ 5, regresja ∈ [0, 1], budżet zapytań > 0); brak API zmiany polityki.
- Błąd uruchomienia wariantu = niezaliczone powtórzenie (zachowawczo).

## Zdolności / uprawnienia
`fs.read(evals/**)`, `fs.read(%LOCALAPPDATA%/Alfa/evals/holdout/**)` (katalog Jądra — poza zasięgiem narzędzi agentek; deny-lista Brokera do potwierdzenia przez człowieka).

## Izolacja
`inproc`, `on-demand` (oceny w bezczynności).

## Budżet zasobów
RAM ≤ 8 MB (bez uruchamianego wariantu); bootstrap 2000 × 1000 przypadków < 50 ms.

## Konfiguracja (klucze TOML)
`[evals] root = "evals"`, `holdout = "%LOCALAPPDATA%/Alfa/evals/holdout"`, `[evals.gate] min_repeats = 5`, `min_cases = 10`, `max_regression = 0.0`, `min_improvement = 0.0`, `max_holdout_queries = 20`, `budget_window = "24h"`, `report_decimals = 3` — polityka bramki jest polityką Jądra (`kernel.evals.*`, zmienia tylko Broker).

## Wkład do UI
Panel „Zdrowie systemu” → wyniki bramki (werdykty zbiorcze), lista zestawów z integralnością; raporty Markdown w PR.

## Testy akceptacyjne
- `ACC-F8-evals-01` (F8-03): holdout tylko przez bramkę, werdykt bez danych przypadków, N < 5 odrzucone, budżet zapytań, zmieniony plik holdoutu blokuje — `contract_tests::run_gate_suite` (impl + fake), `evals-impl/tests/holdout.rs`.
- `ACC-F8-evals-02`: zamrożony zestaw ze zmianą = błąd, propozycja = raport rozjazdu — `run_catalog_suite`; repozytorium: `tests/repo_suites.rs` (F8 ściśle, F2/F3/F5/F7 jako przykłady formatu).
- Property-based: przedział bootstrap w [min, max] i deterministyczny.

## Fake
`evals-fake`: `FakeCatalog` (zestawy w pamięci, `tamper`), `FakeGate` (holdout w pamięci, wirtualny zegar, rejestr żądań); runner `QualityRunner` w `contract_tests`.

## Otwarte pytania
- Replay offline na własnych logach jako `CandidateRunner` (PLAN §12.4) — podpięcie w `app-*` z `core-log` i `providers` (record/replay).
- Kopiowanie zestawów do `evals/acceptance/` + `HASHES` przy zamrożeniu — decyzja człowieka (dziś zamrożenie przez `status: frozen` w manifeście).

## Fala 5
- `alfa-evals verify evals` kończył się błędem „powtórzony zestaw `f5`”: stary format manifestu (`LegacyManifest`)
  brał identyfikator tylko z `wave`, a `evals/F5/voice/MANIFEST.json` ma `set: "voice"`. Teraz identyfikator =
  `<fala>-<set>` (`f5-voice`); zestaw `f5` i jego skrót manifestu bez zmian, żaden plik zestawu nie zmieniony.
  Test pilnujący całego katalogu: `evals-impl/tests/repo_suites.rs::alfa_evals_verify_passes_on_repo_catalog`
  (brak problemów katalogu + kod 0 binarki `alfa-evals verify`), jednostkowy: `legacy.rs`
  `legacy_manifest_with_set_gets_its_own_suite_id`.
