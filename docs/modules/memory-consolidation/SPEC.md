# memory-consolidation — SPEC (F7)

## Cel
Strażniczka pamięci (rola Bety w obsadzie „Standard”, PLAN §9.2): nocna konsolidacja pamięci (PLAN §10) —
streszczanie epizodów, ekstrakcja faktów i umiejętności, deduplikacja, sprzeczności, wygaszanie (retencja), propozycje
awansu za zgodą — jako zadanie tła z budżetem, dziennikiem zmian i cofaniem.

## Fala i priorytet
F7 (pamięć pełna). P1.

## Kontrakt (Rust)
```rust
// memory-consolidation-contract
pub struct Guardian; // run(Trigger::{Scheduled, Manual}) -> RunReport; undo_run(memory, run)
pub fn may_start(&ConsolidationConfig, &HostState, Trigger) -> Result<(), SkipReason>;
pub trait HostConditions { fn state(&self) -> HostState }            // bateria, pełny ekran, bezczynność, czas lokalny
#[async_trait] pub trait Consolidator {                               // LLM (ModelProvider) / skryptowana atrapa
    fn model(&self) -> ConsolidatorModel; fn estimate_micro_usd(&self, &ConsolidationBatch) -> Option<u64>;
    async fn consolidate(&self, &ConsolidationBatch) -> Result<ConsolidatorOutput, ConsolidationError>; }
#[async_trait] pub trait BackgroundBudget {                           // cost-meter, background = true
    async fn check(&self, &ConsolidatorModel, Option<u64>) -> BudgetVerdict; async fn record(&self, &LlmUsage) -> …; }
pub mod rules { retention, duplicates, contradictions, proposals_to_ops, subject_of }
```
Zmiany trafiają do pamięci przez `MemoryService::apply_changes` (`Accessor::Guardian`) — każdy rekord dziennika ma
`run`; `undo_run` cofa przebieg (wygaszenia nieodwracalne). Zdarzenia: `memory.consolidation.started/finished/skipped`
(liczniki, bez treści).

## Zależności
`memory-contract`; `-impl`: `providers-contract` (model lokalny), `cost-meter-contract` + `accounts-hub-contract`
(budżet tła), `device-profile-contract` (bateria, pełny ekran), `core-registry-contract` (moduł).

## Niezmienniki
- **Nie startuje na baterii ani w trybie gry/pełnego ekranu** — dotyczy także uruchomienia ręcznego; harmonogram
  dodatkowo: okno nocne (domyślnie 02:00–05:00, przez północ) i bezczynność ≥ 10 min. Stan sprawdzany przed każdym
  zakresem — zmiana przerywa przebieg (`interrupted`), bez propozycji awansu.
- Do modelu trafiają wyłącznie **epizody zaufane**, nieprzetworzone, pierwotne; sesje prywatne tylko modelem lokalnym
  (`require_local_for_private`); treść epizodów w prompcie jest danymi (JSON w `<dane>`), odpowiedź wyłącznie JSON,
  parsowana ściśle; źródła propozycji muszą należeć do wsadu; limity 20 faktów / 5 streszczeń / 5 umiejętności.
- Fakty z modelu: proweniencja agentki `strazniczka-pamieci` (ranga niższa niż użytkownika — nie zastąpi faktu
  użytkownika bez zgody), `auto_extract = ask` → oczekujące; umiejętności zawsze oczekujące; zakresy szersze niż
  sesja — zawsze oczekujące (zgoda).
- Budżet: przed wywołaniem `check` (szacunek mikro-USD → mikro-PLN kursem `cost-meter`); odmowa → pomija model do
  końca przebiegu, reguły deterministyczne działają; model chmurowy o nieznanym koszcie → odmowa; zużycie rejestrowane
  z `background = true`, agentka `beta`.
- Sprzeczność: nowszy fakt zaufany o randze ≥ zastępuje starszy (`Resolve`), inaczej konflikt raz w dzienniku.
- Propozycje awansu: fakt aktywny, zaufany, w ≥ 2 sesjach nieprywatnych, brak w globalnej → kopia oczekująca.

## Zdolności / uprawnienia
Brak tokenów; dostęp do pamięci jako `Accessor::Guardian` (bez Inspektora UI, bez zatwierdzania).

## Izolacja
`inproc`, `on-demand`; zadanie tła tokio (`ConsolidationModule`, sprawdzenie co 15 min).

## Budżet zasobów
RAM ≤ 16 MB (bez modelu); model lokalny przez `model-residency` (priorytet „tło”); budżet tła `cost-meter`
(domyślnie 0 PLN → tylko modele lokalne).

## Konfiguracja (klucze TOML)
`[memory.consolidation] enabled = true`, `window = "02:00-05:00"`, `not_on_battery = true`,
`not_in_fullscreen = true`, `min_idle = "10m"`, `max_episodes = 50`, `dedup_similarity = 0.9`,
`episodic_retention = "180d"`, `propose_promotions = true`, `promotion_min_sessions = 2`; `[memory] auto_extract`.

## Wkład do UI
Inspektor pamięci: „Uporządkuj teraz” (`run_now`), ostatni raport, filtr „oczekujące”, dziennik przebiegu z „Cofnij
przebieg”; Ustawienia → Pamięć → Konsolidacja.

## Testy akceptacyjne
- `ACC-F7-05`: 20 scenariuszy na atrapie `device-profile` (bateria / tryb gry, pory, bezczynność, wyzwalacze) —
  0 startów (`memory-consolidation-impl/tests/adapters.rs`) + 20 na atrapie stanu (`-contract/tests/guardian.rs`).
- `ACC-F7-04`: 50 przebiegów — 0 awansów treści niezaufanej; treść niezaufana i prywatna nie trafia do modelu.
- Reguły: duplikaty (property: grupy rozłączne, wiodący najbardziej zaufany), sprzeczności, retencja, walidacja
  propozycji; budżet; przerwanie między zakresami; cofnięcie przebiegu.

## Fake
`memory-consolidation-fake`: `ScriptedConsolidator` (kolejka odpowiedzi, zapis wsadów), `FixedBudget`, `FakeHost`.

## Otwarte pytania
- Licznik bezczynności (`IdleSource`) — produkcyjnie `GetLastInputInfo` w `platform-windows` (dziś: brak → harmonogram
  nie startuje, ręczne działa).
- Integracja z `scheduler`/`triggers` (dziś własny interwał modułu) i z `model-residency` (rezerwacja modelu tła).
