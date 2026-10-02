# scheduler-contract

Kontrakt pełnego schedulera F5 (docs/modules/scheduler/SPEC.md) — **nadzbiór `scheduler-lite`**:
ta sama tablica blokad zasobów wyłącznych (mowa i zadania), te same typy (`Resource`, `Holder`,
`Priority`, `Lease`), trait `Scheduler: SchedulerLite`.

- `TaskSpec`: węzeł DAG (zależności z warunkami `succeeded/failed/finished/output_equals`, wyniki
  pośrednie `TaskOutput`), delegacja (`parent`, anulowanie poddrzewa), przydział (`Persona`, `Role`,
  `AnyAgent`, `System`), klasa (`User > Agent > Background`), pochodzenie (`TaskOrigin` →
  `LaunchOrigin` dla mostów; dziedziczone przez podzadania), zasoby, okno czasowe, budżety, ponowienia, taint.
- `SchedCore<H: SchedHost>`: deterministyczny rdzeń (przydział atomowy kompletu zasobów przez
  `Core::try_acquire_all` z `scheduler-lite`, rezerwacje, wywłaszczanie niższej klasy i mowy w punktach
  atomowych, budżet tła z `cost-meter` przez host, terminy i przerwanie siłą po `STOP_GRACE_MS`,
  steering, trwałość `Snapshot`, zdarzenia `scheduler.task.*`). `-impl` i `-fake` różnią się tylko hostem.
- `TaskView`: stan, budżety, powód blokady oraz `agent` i `started_at_ms` — także dla zadań zakończonych
  (panel Zadania „kto co zrobił”); stan bez tych pól wczytuje się bez migracji.
- Protokół wykonawczyni: `TaskExecutor::execute(Dispatch, StepGate)`; `StepGate::boundary` po każdym
  kroku atomowym → `Continue{steering}` / `Yield` / `Stop`; `StepGate::spawn` (delegacja).
- Feature `contract-tests`: wspólny zestaw (`run_all`), skryptowana wykonawczyni (`Script`, `ScriptRun`,
  `ScriptedExecutor`).
