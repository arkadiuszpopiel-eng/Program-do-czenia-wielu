# agent-backends-fake

`FakeAgentBackend` — deterministyczny `AgentBackend` w pamięci (scenariusze ze znacznika w poleceniu,
prawdziwe reguły pochodzenia, `ApprovalSink`/`approve`, sterowanie, anulowanie) dla testów innych
modułów (tylko `dev-dependencies`). Binarium `alfa-fake-agent-cli` udaje `claude -p` (stream-json,
prośby o uprawnienia przez kanał MCP Alfy) i `codex app-server` (JSON-RPC, zatwierdzenia), w tym
crash, kod ≠ 0, śmieci na stdout, bardzo długie linie i proces-wnuk. Testy w `tests/` uruchamiają
prawdziwe mosty z `agent-backends-impl` na tym binarium (kontrakt, F4-01/02/03/05, F5-04).
