# agent-runtime-impl

Implementacja `agent-runtime` (docs/modules/agent-runtime/SPEC.md): `Runtime` = `AgentRuntime` v1
(`module.toml`: `on-demand`, `inproc`, RAM ≤ 10 MB na przebieg).

- v0: pętla plan → narzędzie → obserwacja → weryfikacja, budżety, checkpointy (`DirCheckpointStore`),
  anulowanie, detektor pętli, steering, zdarzenia `agent.*`, niezaufana treść z delimitacją i taintem.
- v1: przebiegi równoległe z zasobami wyłącznymi (`RuntimeExt::locks` — `scheduler-lite`), granica kroku
  przed każdym wywołaniem narzędzia (`StepGate::boundary`, sterowanie ≤ 1 krok), paczki równoległych
  odczytów, delegacja (`plan_delegation`: koperta potomka ≤ rodzica, ta sama sesja, taint w obie strony,
  sufit autonomii z `BrokerAutonomy`), Krytyczka (`Cast::verifier_for`, rola tylko do odczytu),
  raport końcowy, adapter schedulera `RuntimeExecutor` (`TaskExecutor`, ładunek `AgentTaskPayload`).

Zależności produkcyjne: tylko `*-contract`. Testy: v0 (`contract`, `control`, `loop_flow`, `props`) oraz v1
(`parallel` — 3 agentki i 100 scenariuszy bez kolizji, `delegation` — własność 512 przypadków,
`critic`, `steering` — F5-02 20/20, `executor`).
