# agent-runtime — SPEC (v1, zaimplementowany; v0 zgodne wstecz)

## Cel
Pętla agentki: plan → działanie → obserwacja → weryfikacja; dziennik zdarzeń (event-sourced), checkpointy, budżety (kroki/tokeny/czas/koszt), anulowanie, detektor pętli, steering między atomowymi krokami. v0 (F3): jedna agentka, narzędzia sekwencyjnie. **v1 (F5):** wiele przebiegów równolegle, granica kroku dla schedulera, równoległe odczyty, delegacja, Krytyczka, raport końcowy (PLAN §9.1–9.2, §9.6, §16.2).

## Fala i priorytet
F3: v0. F5: v1 (ten dokument). P0.

## Kontrakt (`agent-runtime-contract`; zmiany v1 wyłącznie addytywne)
```rust
pub struct RunSpec { session, agent, persona, roles, goal, origin, model, tools, budget: RunBudget, workdir, verify, approval_timeout_ms, history }
pub enum Steer { Message(String), PauseAfterCurrent, Resume, ChangeGoal(String), Cancel }
pub enum RunOutcome { Completed { summary, verified }, BudgetExceeded { budget }, Cancelled, LoopDetected { tool }, Refused, Failed { error } }
// v1
pub struct RunOptions { parent: Option<RunId>, depth, grant: Option<RunGrant>, crew: Option<Crew>, inherited_taint, trusted_context, untrusted_context, label }
pub struct RunGrant { tools, capabilities, read_only, budget, max_autonomy }   // attenuate(): potomek ≤ rodzic; is_within(); permits()
pub struct Crew { cast: Cast, personas, roles, models: BTreeMap<RoleId, String> }
pub struct DelegateArgs { role, persona?, goal, tools?, max_steps? }           // narzędzie `delegate_task` (grupa ról `delegate`)
pub struct AgentTaskPayload { spec: RunSpec, options: RunOptions }            // TaskSpec::payload dla schedulera
pub struct RunReport { run, goal, outcome, done, not_done, undoable, verification, steering, tainted, usage, children } // from_events, total_usage, summary_pl
pub fn steps_before_delivery(events, sent_after_seq, message) -> Option<u32>  // miara F5-02
#[async_trait] pub trait AgentRuntime { start, steer, cancel, resume, wait, status, events, subscribe,
    async fn start_with(spec, RunOptions); fn children(&RunId); fn report(&RunId) -> RunReport }   // v1: metody z domyślną implementacją
```
Zdarzenia `agent.*` **bez nowych wariantów** (UI/Replay v0 działa): delegacja = krok narzędzia `delegate_task` (wynik niesie id podprzebiegu), Krytyczka = krok `Verify` + `Verified{note: "Gama (Krytyczka): …"}`, oddanie zadania schedulerowi = `Paused` → `Resumed`. Checkpoint niesie `options`, `taint_source`, `delegated` (serde default — stare checkpointy się wczytują).

## Model v1 (`agent-runtime-impl`)
- **Równoległość:** każdy przebieg = własny uchwyt (dziennik, kolejka sterowania, token anulowania, budżet, taint). Narzędzia zmieniające stan biorą dzierżawy `scheduler-lite` (`RuntimeExt::locks`: pliki ze ścieżek argumentów, katalog powłoki, `ScreenInput` dla `gui.control`) w porządku kanonicznym, z limitem `lease_wait_ms` i anulowaniem; `AlreadyHeld` (zasób zadania schedulera) = bez nowej dzierżawy.
- **Granica kroku:** przed każdą turą modelu **i każdym wywołaniem narzędzia**: `StepGate::boundary(StepReport{koszt mikro-PLN, odcisk})` → `Continue{steering}` (treść do kolejki) / `Yield` (checkpoint, `Paused`, wznowienie tym samym przebiegiem) / `Stop` (wynik); potem pauza/anulowanie; nowa wiadomość właściciela **pomija resztę wywołań tury** („przeplanuj”) → trafia do następnej tury (≤ 1 krok). Sterowanie rodzica przekazywane aktywnemu podprzebiegowi.
- **Narzędzia równoległe:** kolejne wywołania tylko do odczytu w turze = jedna paczka (`join_all`, ≤ `max_parallel_reads`), zapisy i delegacja szeregowo.
- **Delegacja:** `plan_delegation` (czysta): rola musi być w obsadzie (agentka gra rolę), koperta potomka = `parent.attenuate(rola ∩ żądane narzędzia)`; koperta rodzica = `spec.tools` ∩ koperta v1 (przed filtrem ról zlecającej — Dyrygentka zleca, Wykonawczyni pisze), budżet = reszta rodzica, sufit autonomii (`AutonomyOracle`/`BrokerAutonomy`: wykonawczyni z wyższym poziomem = odmowa), ta sama sesja i pochodzenie polecenia, taint i proweniencja w dół, wynik skażonego potomka niezaufany dla rodzica, zużycie potomka liczone do budżetu rodzica, głębokość ≤ `max_delegation_depth`.
- **Krytyczka:** gdy `verify` i jest obsada: `Cast::verifier_for(autorka)` → Krytyczka (≠ autorka) albo zastępczyni; podprzebieg w roli Krytyczki (`read_only`, tylko narzędzia niezmieniające stanu z koperty autorki, budżet `critic_budget`), wynik i kroki autorki w bloku `<<<NIEZAUFANE`; „WERYFIKACJA: BŁĄD” → wiadomość do autorki i poprawka (≤ `max_verify_rounds`), potem `Completed{verified}`. Samoweryfikacja v0 tylko bez obsady albo w Solo.
- **Adapter schedulera:** `RuntimeExecutor: TaskExecutor` — ładunek `AgentTaskPayload`, przebieg `task-<zadanie>-a<próba>` (wznowienie z checkpointu), agentka przydzielona przez scheduler z obsady, budżety ∩ zadanie, taint zadania, pochodzenie ≠ użytkownik → `CommandOrigin::Agent`; `Completed` → `Succeeded{run, verified}`, odrzucenie Krytyczki/pętla/odmowa → `Failed{retryable:false}`, błąd dostawcy → ponawialny, `Stop`/anulowanie → `Stopped`; porzucenie wykonania anuluje przebieg.

## Niezmienniki
- Każdy krok to zdarzenie; stan odtwarzalny z dziennika; `StepStarted` = `usage.steps`; checkpoint po każdej turze.
- Narzędzie tylko przez Brokera (token jednorazowy); odmowa = `Denied`, nigdy obejście. Potomek ≤ rodzic (koperta, budżet, autonomia, sesja).
- Taint monotoniczny w przebiegu i dziedziczony; argumenty z niezaufanej treści oznaczane (`untrusted_args`).
- Agentka nie podnosi autonomii ani nie zmienia Jądra; Krytyczka nigdy nie dostaje narzędzi zmieniających stan.

## Zdolności / izolacja / budżet
Brak własnych zdolności (tokeny per akcja). `inproc`, `on-demand`. RAM ≤ 10 MB per przebieg; narzut pętli ≤ 20 ms na krok.

## Integracja (`app-*`, opis — podpina sesja kompozycji)
`Runtime::with_ext(deps, RuntimeExt{ locks: scheduler (SchedulerLite), autonomy: BrokerAutonomy(broker) })`; `start_with(spec, RunOptions{crew: obsada sesji z personas, models wg polityki ról})`; `RuntimeExecutor::new(&runtime, kurs)` jako wykonawczyni `ExecutorKind::Agent` w `SchedulerModule`; panel Agentki/Replay: `children` + `report` (kroki cofalne całego zadania, koszty).

## Testy akceptacyjne
- `ACC-F3-agent-runtime-01..03` (v0): `app-core/tests/agents.rs`, `control.rs`, `loop_flow.rs`, `props.rs` — zielone bez zmian.
- `ACC-F5-agent-runtime-04` (F5-02): `tests/steering.rs` — 20/20 (10 tekst, 10 głos przez `StepGate`).
- `ACC-F5-agent-runtime-05` (F5-01 w runtime): `tests/parallel.rs` — 3 agentki równolegle, 0 kolizji (+ 100 losowych scenariuszy, kontrola negatywna bez dzierżaw).
- `ACC-F5-agent-runtime-06`: `tests/delegation.rs` — własność „delegacja nie rozszerza uprawnień” (512 przypadków), odmowy, taint w obie strony; `tests/critic.rs` — odrzucenie → poprawka; `tests/executor.rs` — Yield/Stop/wznowienie.
- Kontrakt v1 (`contract_tests_v1`) na `-impl` i `-fake`.

## Fake
`agent-runtime-fake`: przebiegi ze skryptu zdarzeń na wirtualnym zegarze; v1: `start_with` z zapisem opcji, `children` (z `options.parent`), `report`.

## Otwarte pytania
- Szyfrowany magazyn checkpointów (wznowienie po restarcie w aplikacji).
- Delegacja jako podzadania schedulera (`StepGate::spawn`) zamiast podprzebiegu — gdy podzadanie ma trwać dłużej niż krok rodzica.
