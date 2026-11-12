# router — SPEC (v1, F1)

## Role lokalne — korekta właściciela 12.11.2026

Kod i Planning oraz rozmowa/GUI z jawną trudnością Hard lub istniejącymi sygnałami
kodu/kontekstu kodu wymagają wybranego profilu Qwen3.8-27B Q4 albo dopuszczonej chmury.
Bramka roli jest niezależna od rankingu: słaby lokalny model odpada także z fallbacków,
przypięcia, review i przy wyłączonym doborze trudności. Profil ma dokładny identyfikator
`qwen3.8-27b-ud-q4-k-m` albo `qwen3.8-27b-q4-0` i wpis możliwości w rejestrze adaptera.
Sama liczba parametrów lub ręczny szczebel Strong nie daje kwalifikacji. Lista nie
poświadcza jakości; adapter nadal weryfikuje model/dostępność, a osobny odbiór sprzętu,
pełny hash i testy kodowania pozostają wymagane. Brak zasobów/trasy → czytelny błąd.
Automatyczny dobór lokalny odkrywa profil także obok małego modelu domyślnego.
Jawne kolejki i eskalacja zachowują kolejność, ale nie omijają bramki. Prosta rozmowa,
streszczenia i embeddingi zachowują dotychczasowe role. Prywatność, możliwości,
zdrowie, budżet i zakaz powtarzania rozpoczętych efektów bez zmian.

Rola `research` oznacza Conversation: proste WWW może użyć małego modelu. Sam kod
w wyszukiwanej frazie jest tematem, nie poleceniem programowania. Jawne Hard lub
oszacowanie trudności Hard nadal wymagają profilu eksperckiego; typowane Code/Planning
nie korzystają z wyjątku research. Wskazówka zadania
nie może ukryć kodu w samym żądaniu, a eskalacja po profilu eksperckim zachowuje rolę.

Starszy zapis poniżej opisuje stan z10.11; nowa reguła ma pierwszeństwo dla kodu.

## Historyczne role modeli — zlecenie właściciela10.11.2026

Automatyczna główna praca na profilu lokalnym8–12B zaczyna się lokalnie; lekka chmura
Luna/Haiku/odpowiednik z dostępnego konta jest pomocą przy problemie. Brak modelu lub konta
nie daje fikcyjnego sukcesu. Trudne zadania/eskalacja, ręczne kolejki/przypięcia i istniejące
bramki prywatności, możliwości i budżetu pozostają nadrzędne.1.5B local-light tylkoSummarize.
Jawny lekki szczebel działa także dla firmowego aliasu API. Kolejka konta z włączonym
przestawianiem zachowuje dobór według trudności; pomoc chmury nie zmienia jej szczebli.


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
  interaktywne 1,5 s, tło 5 s, plus 250 ms na 1000 tokenów wejścia ponad 8 tys., najwyżej 30 s — od 0.0.69; nie dla ostatniego celu)
  → natychmiast następny cel z tą samą historią; zdarzenia przed
  treścią buforowane (jedno `Started`). Błąd **po** treści (`after_output`) — bez cichego przełączenia: konsument dostaje
  błąd, a tura zawiera częściową odpowiedź.
- Anulowanie jest obserwowane także przez Router podczas oczekiwania na zdarzenie. Zawieszony
  adapter ignorujący token nie blokuje zakończenia trasy. Router zwalnia jego strumień, raportuje
  `Cancelled` (zwolnienie próby half-open bez naliczenia awarii), kończy jednym `Stop(Cancelled)`
  i nie uruchamia celu zapasowego. Wysłana treść i rozpoczęte wywołanie narzędzia nie są powtarzane.
- Przełączają też błędy `InvalidRequest` dotyczące samego modelu, nie treści: model nieistniejący albo bez narzędzi
  (`model_unavailable`) i **żądanie za długie dla okna modelu** (`context_overflow`: llama-server
  `exceed_context_size_error`, OpenAI `context_length_exceeded`) — następny cel, zwykle z większym oknem (1.11).
- Obwód per dostawca: `failures` błędów w `window` → otwarty na `cooldown`, potem jedna próba half-open. Nie liczą się:
  `InvalidRequest`, `PrivacyBlocked`, `Unsupported`, anulowanie; 429 → osobne okno (reaktywnie: `retry-after`, bez nagłówka
  30 s × 2ⁿ ≤ 1 h).
- Automatyczna pula kandydatów powstaje z modeli lokalnych i skonfigurowanych kont API.
  Zanim powstanie decyzja, bramka roli usuwa niedopuszczone lokalne modele dla kodu/trudnej pracy;
  brak właściwego modelu i chmury daje NoRoute, także bez kluczy. Prosta rozmowa oraz zadania
  pomocnicze mogą pozostać lokalne. Nowy klucz (stan ≠ Unconfigured) widoczny bez restartu.

## Zależności
`core-bus/config/log-contract`, `accounts-hub-contract` (stany kont), `compliance-contract` (status tras, tagi), `cost-meter-contract` (budżety), `providers-api/local-contract` (capabilities, health), `agent-backends-contract` (F4).

## Niezmienniki
- Deterministyczny: ta sama polityka + stan = ta sama decyzja (testowalne; bez LLM w pętli).
- Sesja „prywatne" nigdy nie kieruje do trasy CN / „może trenować"; sesja `tainted` → `net.egress` wymaga potwierdzenia (Broker, F3).
- Trasa wyłączona w rejestrze zgodności lub konto `Unconfigured` = niewidoczna dla routera (0 wywołań).
- Brak klucza nie uprawnia do kodowania na małym modelu. Bez dozwolonego profilu lokalnego
  kodu i trasy chmurowej zadanie jest jawnie zatrzymywane; prostsze zadania zachowują tryb offline.
- Fallback nie gubi wiadomości: żądanie powtarzane na kolejnym celu z tą samą historią.
- Wagi routera zmienia tylko `improver` w R0 po bramce ewaluacyjnej; tagi/budżety — nigdy.

## Zdolności / uprawnienia
Brak własnych.

## Izolacja
`inproc`, `always` (krytyczny wg §3.1).

## Budżet zasobów
Decyzja ≤ 1 ms; RAM ≤ 2 MB; brak I/O w ścieżce decyzji.

## Konfiguracja (klucze TOML)
`[router.class.<klasa>] prefer = ["anthropic:claude-opus-5-5", "local:bielik-4.5b-v3.0-instruct-q8_0"]`, `[router] speaker = "alfa"`, `thinker = "gama"`, `[router.breaker] failures = 3`, `window = "60s"`, `cooldown = "60s"`, `[router.deadline] voice_fast = "1200ms"` (`"off"` wyłącza). Polityka prywatności sesji (CN, „może trenować", `unknown`) — `compliance` (`PrivacyPolicy`, kernel_policy).

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
- Ustalone (0.0.11): `set_light_local` — lekki dostawca lokalny jest pierwszy w klasach `LIGHT_CLASSES` (dziś: streszczenia) i nie występuje w pozostałych; gdy jest niedostępny, kolejka idzie dalej jak dotąd.
