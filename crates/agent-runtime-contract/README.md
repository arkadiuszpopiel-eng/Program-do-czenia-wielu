# agent-runtime-contract

agent-runtime: kontrakt pętli agentki (plan → akcja → obserwacja → weryfikacja) — RunSpec z budżetami, zdarzenia agent.* dla UI/Replay, steering, checkpointy, trait AgentRuntime, testy kontraktowe.

Kontrakt modułu `agent-runtime`: trait, typy, zdarzenia i współdzielone testy kontraktowe. Inne moduły zależą wyłącznie od tego crate'a.

Specyfikacja: [`docs/modules/agent-runtime/SPEC.md`](../../docs/modules/agent-runtime/SPEC.md).

```bash
cargo test -p agent-runtime-contract
```
