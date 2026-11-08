# agent-backends-contract

Kontrakt `AgentBackend` (PLAN §5.1, §8.5, ADR 0005; SPEC: `docs/modules/agent-backends/SPEC.md`):
`TaskSpec`/`TaskHandle`, zdarzenia `AgentEvent` w kopercie `AgentEventEnvelope` (zawsze
`unverified_by_alfa = true`), prośby o uprawnienia i port `ApprovalSink`, port `Workspace`, błędy
`BackendError`. Moduł `policy` zawiera czyste reguły zgodności wspólne dla `-impl` i `-fake`:
`check_origin` (wyzwalacz i Ulepszacz — nigdy; harmonogram — tylko z jawną zgodą i limitem),
`check_version` (przypięte wersje CLI), `child_env` (środowisko CLI bez sekretów Alfy).
Feature `contract-tests`: zestaw testów kontraktowych i scenariusze atrap (`[alfa-fake:<nazwa>]`).
