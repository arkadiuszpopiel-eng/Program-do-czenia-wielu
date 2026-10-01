# router — SPEC (v1, F1)

## Cel
Deterministyczny wybór trasy dla zadania: klasa zadania (głos-szybka, rozmowa, kod, planowanie, GUI/wizja, streszczanie, embeddingi) × ograniczenia (tag prywatności i jurysdykcji, budżet, opóźnienie, możliwości) → dostawca/model lub backend agentowy; fallback, circuit breaker, limity okien planów; Mówczyni + Myślicielka (PLAN §5.4). Nie wywołuje modeli sam — zwraca decyzję i nadzoruje fallback.

## Fala i priorytet
F1: v1 (klasy zadań, tagi, fallback, circuit breaker, ModelProvider). F4: AgentBackend (mosty). Uczenie z wyników przez §12 — F8. Tryb „Rada" — P2. P0.

## Kontrakt (v1 — `crates/router-contract`)
```rust
pub use accounts_hub_contract::TaskClass; // VoiceFast, Conversation, Code, Planning, GuiVision, Summarize, Embeddings
pub struct Candidate { pub provider: ProviderId, pub model: String }        // tekstowo "dostawca:model"
pub enum RouteKind { Local /* bez rejestru zgodności i jurysdykcji */, Api /* trasa "<dostawca>.api" */ }
pub struct Constraints { pub session: SessionTag, pub jurisdiction_allow: Vec<String>, pub max_latency_ms: Option<u64>,
                         pub needs: CapabilityNeeds, pub background: bool, pub pinned: Option<Candidate>, pub tainted: bool }
pub struct RouteDecision { pub class: TaskClass, pub chosen: Candidate, pub fallbacks: Vec<Candidate>,
                           pub rejected: Vec<(Candidate, RejectReason)>, pub warnings: Vec<(Candidate, RouteWarning)> }
pub enum RouteError { NoRoute { class: TaskClass, rejected: Vec<(Candidate, RejectReason)> } }
pub trait Router: Send + Sync {
    fn route(&self, class: TaskClass, c: &Constraints, request: Option<&ChatRequest>) -> Result<RouteDecision, RouteError>;
    fn report(&self, candidate: &Candidate, outcome: Outcome /* Ok{ttft,latency} | Failed{kind} | Cancelled */);
    fn breaker_state(&self, provider: &ProviderId) -> BreakerState;   // Closed | Open{until} | HalfOpen{trial_in_flight}
    fn policy(&self) -> RoutePolicy;   // classes, first_event_ms per klasa, breaker, duo (Mówczyni+Myślicielka)
}
pub trait BudgetGate { fn check(&self, provider: &ProviderId, estimate_micro_usd: u64, background: bool) -> BudgetDecision; }
```
Kolejność oceny kandydata: rejestr → klucz (`Unconfigured` niewidoczny; 401 → `AuthFailed`) → [API] `route_allowed`
+ jurysdykcja z tagów rejestru + `check_privacy` profilu adaptera → możliwości modelu (`ModelCapabilities`; model
nieznany = ostrzeżenie) → obwód → okno 429 → TTFT (`max_latency_ms`) → [API] budżet (`cost_meter_contract::evaluate`).
Router jest też `ModelProvider` (`router-impl::RoutedProvider`, dekorator per klasa): `model = "auto"` albo przypięcie
`dostawca:model`; `Started::model` = `dostawca:model` (pochodzenie bloków myślenia przywracane przed wysłaniem).

Zdarzenia (bez treści): `router.decision`, `router.fallback` (z → na, przyczyna: błąd/termin, czas), `router.breaker.opened/closed`,
`router.no_route` (czego brakuje — dla UI „dodaj klucz"), `router.plan_window.exhausted`.

### Fallback i obwody (v1)
- Błąd z `should_fallback()` **przed pierwszą treścią** albo brak pierwszego zdarzenia w terminie klasy (głos 1,2 s,
  interaktywne 1,5 s, tło 5 s; nie dla ostatniego celu) → natychmiast następny cel z tą samą historią; zdarzenia przed
  treścią buforowane (jedno `Started`). Błąd **po** treści (`after_output`) — bez cichego przełączenia: konsument dostaje
  błąd, a tura zawiera częściową odpowiedź.
- Obwód per dostawca: `failures` błędów w `window` → otwarty na `cooldown`, potem jedna próba half-open. Nie liczą się:
  `InvalidRequest`, `PrivacyBlocked`, `Unsupported`, anulowanie; 429 → osobne okno (reaktywnie: `retry-after`, bez nagłówka
  30 s × 2ⁿ ≤ 1 h).
- Polityka domyślna (automatyczna z zarejestrowanych dostawców): bez kluczy API wszystko na model lokalny (profil A);
  z kluczem — rozmowa/kod/planowanie/wizja/streszczanie przez API (lokalny jako fallback/offline), głos-szybka i embeddingi
  najpierw lokalnie. Nowy klucz (stan konta ≠ `Unconfigured`) widoczny bez restartu.

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
`[router.class.<klasa>] prefer = ["anthropic:claude-opus-5-5", "local:bielik-4.5b-v3.0-instruct-q4_k_m"]`, `[router] speaker = "alfa"`, `thinker = "gama"`, `[router.breaker] failures = 3`, `window = "60s"`, `cooldown = "60s"`, `[router.deadline] voice_fast = "1200ms"` (`"off"` wyłącza). Polityka prywatności sesji (CN, „może trenować", `unknown`) — `compliance` (`PrivacyPolicy`, kernel_policy).

## Wkład do UI
Chip profilu modelu/„Hybryda" w pasku i composerze, `/model`, Ustawienia → Router i reguły, powód decyzji w szczegółach wiadomości, komunikat „brakuje: klucz X".

## Testy akceptacyjne
- `ACC-F1-router-01`: property-based — dla losowych polityk i stanów kont decyzja spełnia wszystkie ograniczenia (prywatność, jurysdykcja, capabilities) 100%.
- `ACC-F1-router-02`: sztuczny 5xx/timeout → fallback ≤ 2 s bez utraty wiadomości.
- `ACC-F1-router-03`: trasa wyłączona / bez klucza → 0 wywołań w 100 próbach.

## Fake
`router-fake`: zwraca skryptowane trasy (per klasa) i nagrywa `report()` — dla testów `agent-runtime`, `voice-dialog`.

## Otwarte pytania
- Estymata okien planów mostów CLI (reaktywna, jak dla 429 API) — F4, razem z `AgentBackend`.
- Orkiestracja Mówczyni + Myślicielka (`DuoConfig`, `Tempo`) — `agent-runtime`.
- Format `Weights` dla uczenia z wyników — do ustalenia w SPEC v1 razem z `improver` (F8).
