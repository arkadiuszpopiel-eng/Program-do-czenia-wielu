# scheduler — SPEC (v1: kontrakt zaimplementowany)

## Cel
Deterministyczny szeregowacz zadań agentek (PLAN §9.3, §9.6): DAG zależności z warunkami i wynikami pośrednimi, delegacja (drzewo podzadań, anulowanie poddrzewa), przydział agentek wg ról i dostępności, równoległość z wyłącznością zasobów, priorytety, okna czasowe, budżety, ponowienia, steering, trwałość. **Nadzbiór `scheduler-lite`** — ta sama tablica blokad dla mowy (`acquire/preempt/handoff`) i zadań. Nie uruchamia modeli ani narzędzi — robi to wykonawczyni za portem.

## Fala i priorytet
F5, P1. Zastępuje `scheduler-lite-impl` w kompozycji (kontrakt `-lite` bez zmian, tylko rozszerzenia addytywne).

## Kontrakt (źródło prawdy: `crates/scheduler-contract`)
```rust
pub struct TaskSpec { id, title, parent, deps: Vec<Dependency{task, condition: Succeeded|Failed|Finished|OutputEquals}>, assignee: Persona|Role|AnyAgent|System,
  class: User|Agent|Background, origin: TaskOrigin /* User|Agent|Trigger{depth}|Schedule|Improver|System → LaunchOrigin */, executor: Agent|Bridge|Service,
  resources: Vec<Resource>, window: {not_before, deadline, only_when_idle, not_in_game_mode}, budget: {max_steps, max_wall_ms, max_cost, estimated_cost}, retry, taint, payload }
#[async_trait] pub trait Scheduler: SchedulerLite { fn submit(Vec<TaskSpec>); fn cancel(&TaskId, reason) -> Vec<TaskId> /* poddrzewo */; fn steer(&TaskId, Steer) -> u64;
  fn pause/resume; fn task/tasks -> TaskView; fn set_roster(Roster); fn set_conditions(SystemConditions); async fn wait(&TaskId) -> Termination; }
#[async_trait] pub trait TaskExecutor { async fn execute(&self, Dispatch, Arc<dyn StepGate>) -> WorkerResult; }
pub trait StepGate { fn boundary(StepReport) -> StepDirective /* Continue{steering} | Yield | Stop */; fn spawn(Vec<TaskSpec>); }
```
Rdzeń `SchedCore<H: SchedHost>` w kontrakcie; `-impl`/`-fake` różnią się otoczeniem (zegar, zdarzenia, zapis, budżet tła). Zdarzenia: `scheduler.task.{submitted,blocked,dispatched,step,steered,yielded,paused,resumed,retry_scheduled,finished,aborted,loop_suspected,budget_warning,steer_unconsumed}`, `scheduler.kill_switch`, `scheduler.restored` (+ `scheduler.lease.*` z `-lite`). `finished` niesie pochodzenie, taint i klasę.

## Zależności
`scheduler-lite`, `personas`, `safety-broker` (taint), `agent-backends` (`LaunchOrigin`), `cost-meter` (decyzja budżetu), `core-bus`, `core-registry` (`-contract`).

## Niezmienniki
- Zadanie dostaje **komplet** zasobów atomowo albo nic (`Core::try_acquire_all`) i nigdy nie czeka, trzymając zasób → brak cykli oczekiwania; zasoby trzyma tylko zadanie w toku.
- Graf zależności+delegacji acykliczny (zgłoszenie całość albo nic); start dopiero po zakończeniu zależności.
- Każde zadanie ma skończony termin (domyślnie +24 h), budżet czasu/kroków i liczbę prób → kończy się w skończonym czasie z jawnym `Termination`.
- Wywłaszczanie (mowa użytkownika, zadanie wyższej klasy na zasobie wywłaszczalnym, pauza, utrata okna) tylko w punkcie atomowym; po `STOP_GRACE_MS` = 2 s bez punktu atomowego — przerwanie siłą (`Abort` przed zwolnieniem zasobów).
- Steering dostarczany w najbliższym punkcie atomowym (≤ 1 krok); nieodebrany po ostatnim kroku → `steer_unconsumed`.
- Podzadania dziedziczą pochodzenie i taint (nie da się „wyprać” wyzwalacza); most tylko z `User`/`Schedule`.
- Restart = wznowienie: zadania w toku wracają do kolejki (`interrupted`, `resume_from_step`); kill-switch anuluje wszystko i odbiera dzierżawy mowy.

## Zdolności / uprawnienia
Brak (tokeny pobiera wykonawczyni z Brokera przy wykonaniu).

## Izolacja
`inproc`, `always`.

## Budżet zasobów
RAM ≤ 8 MB (≤ 4096 aktywnych zadań, zakończone usuwane po 24 h); decyzja ≤ 1 ms; zapis stanu w tle (łączony).

## Konfiguracja (klucze TOML)
`[scheduler] roster.max_parallel_total = 4`, `max_parallel_system = 2`, `default_deadline = "24h"`, `state_path`; budżet tła = `[cost] background` (`cost-meter`).

## Wkład do UI
Panel Agentki (kto co robi, zasoby, kolejka, powód blokady), Oś czasu (`scheduler.task.*`), kapsuła aktywności, „Pauza/Wznów/Anuluj/Napisz do zadania”.

## Testy akceptacyjne
- `ACC-F5-scheduler-01` (F5-03): 0 zakleszczeń w 1000 losowych scenariuszy — `scheduler-fake/tests/props.rs`, zestaw `evals/F5/scheduler-scenarios.json`.
- `ACC-F5-scheduler-02` (F5-01): agentki równolegle z blokadą ekranu/głośnika, 0 konfliktów w 100 — `tests/parallel.rs`.
- `ACC-F5-agent-runtime-04` (F5-02): steering ≤ 1 krok atomowy 20/20 — `tests/steering.rs`.
- Kontrakt (fake + impl): DAG/warunki, anulowanie poddrzewa, wyłączność, priorytety, voice-first, okna, ponowienia, budżety, restart, kill-switch.

## Fake
`scheduler-fake`: wirtualny zegar, skryptowane wykonawczynie, nagrane zdarzenia, restart ze stanu, decyzja budżetu tła.

## Otwarte pytania
- Magazyn stanu szyfrowany (`lib-sqlstore`) zamiast pliku JSON — ładunki zadań mogą zawierać cele użytkownika.
- Starzenie priorytetów tła (dziś: termin kończy oczekiwanie jawnym `Expired`).
