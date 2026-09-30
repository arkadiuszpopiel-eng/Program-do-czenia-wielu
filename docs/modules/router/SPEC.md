# router — SPEC (szkic v0)

## Cel
Deterministyczny wybór trasy dla zadania: klasa zadania (głos-szybka, rozmowa, kod, planowanie, GUI/wizja, streszczanie, embeddingi) × ograniczenia (tag prywatności i jurysdykcji, budżet, opóźnienie, możliwości) → dostawca/model lub backend agentowy; fallback, circuit breaker, limity okien planów; Mówczyni + Myślicielka (PLAN §5.4). Nie wywołuje modeli sam — zwraca decyzję i nadzoruje fallback.

## Fala i priorytet
F1: v1 (klasy zadań, tagi, fallback, circuit breaker, ModelProvider). F4: AgentBackend (mosty). Uczenie z wyników przez §12 — F8. Tryb „Rada" — P2. P0.

## Kontrakt (szkic Rust)
```rust
// router-contract — SZKIC
pub enum TaskClass { VoiceFast, Conversation, Code, Planning, GuiVision, Summarize, Embeddings }
pub struct Constraints { pub privacy: PrivacyTag, pub jurisdiction_allow: Vec<Jurisdiction>, pub budget: Option<Money>,
                         pub max_latency: Option<Duration>, pub needs: Capabilities, pub session: SessionId, pub tainted: bool }
pub struct Route { pub target: Target /* Provider(AccountId, ModelId) | Backend(BackendId) */, pub fallbacks: Vec<Target>, pub reason: String }
pub trait Router: Send + Sync {
    fn route(&self, class: TaskClass, c: &Constraints) -> Result<Route, RouteError /* NoRoute { missing: Vec<Need> } */>;
    fn report(&self, target: &Target, outcome: Outcome /* Ok { latency, cost } | Err { kind } */);
    fn breaker_state(&self, target: &Target) -> BreakerState;
}
pub struct RoutingPolicy { pub class_prefs: Map<TaskClass, Vec<Target>>, pub speaker: PersonaId, pub thinker: PersonaId, pub weights: Weights }
```
Zdarzenia: `router.decided` (klasa, trasa, powód), `router.fallback`, `router.breaker.opened/closed`, `router.no_route` (czego brakuje — dla UI „dodaj klucz"), `router.plan_window.exhausted`.

## Zależności
`core-bus/config/log-contract`, `accounts-hub-contract` (stany kont), `compliance-contract` (status tras, tagi), `cost-meter-contract` (budżety), `providers-api/local-contract` (capabilities, health), `agent-backends-contract` (F4).

## Niezmienniki
- Deterministyczny: ta sama polityka + stan = ta sama decyzja (testowalne; bez LLM w pętli).
- Sesja „prywatne" nigdy nie kieruje do trasy CN / „może trenować"; sesja `tainted` → `net.egress` wymaga potwierdzenia (Broker, F3).
- Trasa wyłączona w rejestrze zgodności lub konto `Unconfigured` = niewidoczna dla routera (0 wywołań).
- Zaawansowana rozmowa domyślnie przez API chmurowe; lokalny 3–4B tylko dla komend, fallbacku i offline — ale bez kluczy lokalny jest jedyną trasą.
- Fallback nie gubi wiadomości: żądanie powtarzane na kolejnym celu z tą samą historią.
- Wagi routera zmienia tylko `improver` w R0 po bramce ewaluacyjnej; tagi/budżety — nigdy.

## Zdolności / uprawnienia
Brak własnych.

## Izolacja
`inproc`, `always` (krytyczny wg §3.1).

## Budżet zasobów
Decyzja ≤ 1 ms; RAM ≤ 2 MB; brak I/O w ścieżce decyzji.

## Konfiguracja (klucze TOML)
`[router.class.<klasa>] prefer = ["anthropic:claude-opus-5-5", "local:default"]`, `[router] speaker = "alfa"`, `thinker = "gama"`, `breaker.failures = 3`, `breaker.cooldown = "60s"`, `[router.privacy] block_tags = ["cn", "may_train"]` (kernel_policy).

## Wkład do UI
Chip profilu modelu/„Hybryda" w pasku i composerze, `/model`, Ustawienia → Router i reguły, powód decyzji w szczegółach wiadomości, komunikat „brakuje: klucz X".

## Testy akceptacyjne
- `ACC-F1-router-01`: property-based — dla losowych polityk i stanów kont decyzja spełnia wszystkie ograniczenia (prywatność, jurysdykcja, capabilities) 100%.
- `ACC-F1-router-02`: sztuczny 5xx/timeout → fallback ≤ 2 s bez utraty wiadomości.
- `ACC-F1-router-03`: trasa wyłączona / bez klucza → 0 wywołań w 100 próbach.

## Fake
`router-fake`: zwraca skryptowane trasy (per klasa) i nagrywa `report()` — dla testów `agent-runtime`, `voice-dialog`.

## Otwarte pytania
- Estymata okien planów mostów (reaktywna) — F4.
- Format `Weights` dla uczenia z wyników — do ustalenia w SPEC v1 razem z `improver` (F8).
