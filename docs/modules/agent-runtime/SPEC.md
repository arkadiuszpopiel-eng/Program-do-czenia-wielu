# agent-runtime — SPEC (szkic v0)

## Cel
Pętla agentki: plan → działanie → obserwacja → weryfikacja; event-sourced; checkpointy; budżety (tokeny/czas/PLN); anulowanie; detektor pętli; steering między atomowymi krokami. v0 (F3): jedna agentka, narzędzia sekwencyjnie, bez DAG; v1 (F5): wiele agentek równolegle, narzędzia równoległe, steering pełny (PLAN §9.1, §9.6, §16.2).

## Fala i priorytet
F3: v0. F5: v1. P0 (v0).

## Kontrakt (szkic Rust)
```rust
// agent-runtime-contract — SZKIC
pub struct RunSpec { pub session: SessionId, pub persona: PersonaId, pub role: RoleId, pub goal: Goal, pub budget: RunBudget { tokens: u64, wall: Duration, pln: Option<Money> },
                     pub autonomy: AutonomyLevel, pub tools: Vec<ToolId>, pub capability_tokens: Vec<CapToken> /* z Brokera */ }
pub enum RunEvent { Planned { steps: Vec<Step> }, StepStarted(StepId), ToolCall { tool: ToolId, args: Value, reversible: Reversibility }, ToolResult { .. },
                    ApprovalRequested(ApprovalId), Checkpoint(CheckpointId), Verified { ok: bool }, Progress { pct: u8, eta: Option<Duration>, cost: Money },
                    LoopDetected, BudgetExceeded, Cancelled, Finished(Outcome) }
pub enum Steer { Message(String), PauseAfterCurrent, UndoLastStep, ChangeGoal(Goal), Cancel }
pub trait AgentRuntime: Send + Sync {
    fn start(&self, spec: RunSpec) -> Result<RunId>;
    fn events(&self, run: RunId) -> Subscription<RunEvent>;
    fn steer(&self, run: RunId, s: Steer) -> Result<()>;          // przyjęte między atomowymi krokami
    fn resume(&self, run: RunId, from: CheckpointId) -> Result<()>;
    fn cancel(&self, run: RunId) -> Result<()>;                    // ≤ 2 s
}
```
Zdarzenia: `run.*` (jak `RunEvent`; strumień Narzędzia i GUI + Audyt dla akcji wrażliwych), `run.narration` (narracja ze zdarzeń, nie z pamięci modelu).

## Zależności
`core-bus/config/log-contract`, `router-contract`, `providers-*-contract` (przez Router), `sessions-contract`, `memory-contract`, `personas-contract`, `safety-broker-contract` (tokeny, zatwierdzenia), `risk-classifier-contract`, `undo-journal-contract`, `cost-meter-contract`, `tools-*-contract` (fs/shell/clipboard w F3), `scheduler-lite-contract`, `notify-contract`.

## Niezmienniki
- Każdy krok to zdarzenie; stan przebiegu odtwarzalny z dziennika (replay), checkpoint przed każdą akcją nieodwracalną.
- Narzędzie wywoływane tylko z tokenem zdolności o zakresie ≥ wymaganym; brak tokenu = `ApprovalRequested`, nigdy obejście.
- Akcja o ryzyku powyżej progu poziomu autonomii → zatwierdzenie w Broker-UI; sesja `tainted` → egress i wysokie ryzyko wymagają potwierdzenia.
- Budżet przekroczony = zatrzymanie z raportem; detektor pętli (powtórzone wywołania/obserwacje) zatrzymuje po N.
- Steering przyjęty w ≤ 1 kroku atomowym; anulowanie ≤ 2 s; „gotowe" dopiero po weryfikacji (Krytyczka lub samoweryfikacja w v0).
- Narracja generowana ze zdarzeń magistrali; raport samonaprawy zadania (co próbowano, co blokuje) przy porażce.
- Agentka nie może podnieść własnego poziomu autonomii ani zmienić polityk Jądra (testy „zmiana Jądra"/„sama-zatwierdza").

## Zdolności / uprawnienia
Wyłącznie tokeny przekazane w `RunSpec` (potomek ≤ rodzic); runtime sam nic nie żąda.

## Izolacja
`inproc`, `on-demand` (pętla), narzędzia wykonywane w izolacji właściwej dla narzędzia (restricted token / AppContainer ≤ L3).

## Budżet zasobów
RAM ≤ 10 MB per przebieg (bez modeli); narzut pętli ≤ 20 ms na krok; postęp raportowany ≤ 1 s opóźnienia.

## Konfiguracja (klucze TOML)
`[agent] default_budget = { tokens = 200000, wall = "15m" }`, `loop_detector.max_repeats = 3`, `verify_before_done = true`, `narration.verbosity = "short"`, `checkpoint.before_irreversible = true` (kernel_policy).

## Wkład do UI
Drzewo Plan → kroki → status (%, ETA, koszt), kapsuła aktywności, kroki narzędzi w wiadomości, karta „czeka na zatwierdzenie", Oś czasu/Replay, toast „Cofnij".

## Testy akceptacyjne
- `ACC-F3-agent-runtime-01`: scenariusz MVP bez kluczy (desktop z emulacją): zadanie fs + Cofnij; eval narzędzi fs/shell na lokalnym modelu ≥ próg z F0.
- `ACC-F3-agent-runtime-02`: „agentka zmienia Jądro / zatwierdza sama siebie" ≥ 100 scenariuszy = 0 sukcesów; red-team injection ≥ 100 przypadków: 0 eskalacji, 0 egressu bez potwierdzenia.
- `ACC-F3-agent-runtime-03`: replay dziennika odtwarza stan przebiegu (property-based); anulowanie ≤ 2 s.
- `ACC-F5-agent-runtime-04`: steering uwzględniony w ≤ 1 kroku atomowym (20/20); agentki równolegle z blokadą ekranu/głośnika.

## Fake
`agent-runtime-fake`: przebiegi ze skryptu zdarzeń (plan, kroki, prośby o zatwierdzenie, pętla, budżet) na wirtualnym zegarze — testy UI, `voice-dialog`, `broker-ui`.

## Otwarte pytania
- Format planu/kroków w IR (wspólny z Osią czasu i Marszałkiem) — do ustalenia w SPEC v1.
- Kompaktowanie kontekstu przebiegu (tu vs `memory`) — do ustalenia w SPEC v1.
