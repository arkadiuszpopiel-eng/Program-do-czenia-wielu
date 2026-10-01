# agent-runtime — SPEC (v0, zaimplementowany)

## Cel
Pętla agentki: plan → działanie → obserwacja → weryfikacja; dziennik zdarzeń (event-sourced), checkpointy, budżety (kroki/tokeny/czas/koszt), anulowanie, detektor pętli, steering między atomowymi krokami. v0 (F3): jedna agentka na przebieg, narzędzia sekwencyjnie, bez DAG; v1 (F5): wiele agentek równolegle, narzędzia równoległe (PLAN §9.1, §9.6, §16.2).

## Fala i priorytet
F3: v0 (ten dokument). F5: v1. P0.

## Kontrakt (`agent-runtime-contract`)
```rust
pub struct RunSpec { session, agent, persona: Persona, roles: Vec<Role>, goal: String, origin: CommandOrigin, model: String,
                     tools: Vec<String>, budget: RunBudget, workdir: Option<String>, verify: bool, approval_timeout_ms: u64, history: Vec<Message> }
pub struct RunBudget { max_steps: u32, max_tokens: u64, max_wall_ms: u64, max_cost_micro_usd: Option<u64>, max_tool_calls_per_turn: u32 }
pub enum Steer { Message(String), PauseAfterCurrent, Resume, ChangeGoal(String), Cancel }
pub enum RunOutcome { Completed { summary, verified: Option<bool> }, BudgetExceeded { budget }, Cancelled, LoopDetected { tool, repeats }, Refused, Failed { error } }
#[async_trait] pub trait AgentRuntime: Send + Sync {
    async fn start(&self, spec: RunSpec) -> Result<RunId, RunError>;
    fn steer(&self, run: &RunId, steer: Steer) -> Result<(), RunError>;      // przyjęte między krokami atomowymi
    fn cancel(&self, run: &RunId) -> Result<(), RunError>;
    async fn resume(&self, run: &RunId) -> Result<(), RunError>;             // z ostatniego checkpointu
    async fn wait(&self, run: &RunId) -> Result<RunOutcome, RunError>;
    fn status(&self, run: &RunId) -> Result<RunStatus, RunError>;
    fn events(&self, run: &RunId) -> Result<Vec<RunEventEnvelope>, RunError>;  // dziennik (replay)
    fn subscribe(&self, run: &RunId) -> Result<broadcast::Receiver<RunEventEnvelope>, RunError>;
}
pub trait CheckpointStore { fn save(..); fn latest(..); fn runs(..) }     // MemCheckpointStore, DirCheckpointStore
```
`RunEventEnvelope { run, seq, at_ms, event }`; zdarzenia na magistrali (`agent.*`): `agent.run.started|planned|steered|paused|resumed|checkpoint|usage|budget_exceeded|loop_detected|verified|finished`, `agent.step.started|waiting_approval|finished`, `agent.session.tainted`. `StepFinished` niesie `status` (`ok|denied|needs_confirmation|failed|cancelled`), wynik skrócony, `untrusted`, `undo: UndoRef`, `intent: ToolIntent`.

Narzędzia: wspólny kontrakt `tools-common` (docs/modules/tools-common/SPEC.md) — manifest, `Tool::call(args, ToolCtx) → ToolOutcome`, protokół Brokera.

## Integracja w aplikacji (`app-agents`, `app-core`)
- Wiadomość do agentki idzie przez przebieg, gdy sesja ma katalog roboczy (`sessions_choose_workdir`) **i** role agentki w obsadzie dają ≥ 1 narzędzie (`ToolManifest::allowed_for`); inaczej zwykła odpowiedź czatu. `turns_continue` zawsze czat.
- `RunSpec` z obsady (persona, role, prompt), historii gałęzi, katalogu roboczego i ustawień `agents.max_steps|max_minutes|max_cost_pln|approval_timeout_s|verify_before_done`; bez okna Brokera czekanie na zgodę ≤ 60 s (i tak nie da się jej udzielić) — potem odmowa `approval_timeout`.
- Projekcja `agent.*` → UI: `AgentStep` (Replay), `AgentRunUpdated`, `ToolCall` (linie kroków z „Cofnij” i intencją), `ApprovalPending` (`broker_window`, `expires_at`), kapsuła aktywności, Oś czasu; odpowiedź końcowa = tura agentki (append-only) z krokami w faktach. Replay trwały w szyfrowanej bazie sesji (`app_agent_runs/steps`, append-only).
- Checkpointy w pamięci (`MemCheckpointStore`): `DirCheckpointStore` zapisuje treść sesji jawnym JSON-em — do czasu szyfrowanego magazynu brak wznowienia po restarcie.
- Steering: `agents_steer` → `Steer::Message`; Stop/Esc i kill-switch → `cancel` przebiegu i Job Objects poleceń (ta sama instancja `ExecPort` co Broker).

## Niezmienniki
- Każdy krok to zdarzenie; stan przebiegu odtwarzalny z dziennika; checkpoint po każdej turze.
- Narzędzie wywoływane tylko przez Brokera (token jednorazowy); brak zgody = `Denied` z powodem dla modelu, nigdy obejście.
- Sesja `tainted` (wynik `untrusted`) → ryzykowne akcje wymagają potwierdzenia; argumenty pochodzące z niezaufanej treści oznaczane (`untrusted_args`).
- Budżet przekroczony / pętla (3 identyczne wywołania w oknie 12) = czyste zatrzymanie z raportem; samoweryfikacja „WERYFIKACJA: OK|BŁĄD” (≤ 2 rundy poprawek).
- Agentka nie podnosi własnego poziomu autonomii ani nie zmienia polityk Jądra (blokady Jądra w Brokerze).

## Zdolności / uprawnienia
Brak własnych — wyłącznie tokeny Brokera wydawane narzędziom per akcja.

## Izolacja
`inproc`, `on-demand`; procesy narzędzi w Job Objects (`tools-shell`).

## Budżet zasobów
RAM ≤ 10 MB per przebieg (bez modeli); narzut pętli ≤ 20 ms na krok.

## Wkład do UI
Replay krok po kroku (panel Oś czasu → Replay: krok, narzędzie, wejście/wyjście, status, czas, „Cofnij krok”), karty „Cofnij” (toast 8 s + karta), „czeka na zatwierdzenie”, „uruchom w terminalu”, steering z pola wiadomości, Ustawienia → Agentki (budżety).

## Testy akceptacyjne
- `ACC-F3-agent-runtime-01`: zadanie fs + Cofnij (`app-core/tests/agents.rs`); eval narzędzi `evals/F3/tools/` (≥ 30 zadań; CI: format + zadania skryptowane, `app-agents/tests/eval_tools.rs`; pomiar na modelu lokalnym ≥ próg z F0).
- `ACC-F3-agent-runtime-02`: odmowa Brokera → powód w wyniku narzędzia; zgoda bez decyzji → odmowa po czasie; kill-switch zatrzymuje pętlę i polecenie; 0 przecieków między sesjami (`spy_modes.rs`).
- `ACC-F3-agent-runtime-03`: replay dziennika odtwarza stan (property-based, `agent-runtime-impl/tests/props.rs`).

## Fake
`agent-runtime-fake`: przebiegi ze skryptu zdarzeń na wirtualnym zegarze.

## Otwarte pytania
- Szyfrowany magazyn checkpointów (wznowienie po restarcie) — v1.
- Format planu/kroków wspólny z Osią czasu i Marszałkiem — v1.
