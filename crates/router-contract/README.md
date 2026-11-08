# router-contract

Kontrakt Routera (docs/modules/router/SPEC.md, PLAN §5.4, §6.9, ADR 0005/0014).

| Obszar | Typy |
|---|---|
| Klasy zadań | `TaskClass` (re-eksport z `accounts-hub-contract`: głos-szybka, rozmowa, kod, planowanie, GUI/wizja, streszczanie, embeddingi) |
| Kandydaci | `Candidate` (`dostawca:model` — konfiguracja, przypięcie w `ChatRequest::model`, `Started::model` Routera), `RouteKind` (`Local` — bez rejestru zgodności i jurysdykcji; `Api` — trasa `<dostawca>.api`) |
| Ograniczenia | `Constraints` (tag sesji `SessionTag`, `jurisdiction_allow`, `max_latency_ms`, `CapabilityNeeds`, `background`, `pinned`, `tainted`), `Constraints::from_request` |
| Decyzja | `RouteDecision { class, chosen, fallbacks, rejected: Vec<(Candidate, RejectReason)>, warnings }`, `RejectReason` (NotRegistered, Unconfigured, AuthFailed, Compliance, Privacy, Jurisdiction, Capability, Budget, CircuitOpen, PlanWindow, Latency), `RouteWarning`, `RouteError::NoRoute`, `Outcome` |
| Obwody i limity | `CircuitBreaker` (N błędów w oknie → otwarty na T → jedna próba half-open), `PlanWindow` (reaktywnie z 429: `retry-after` albo 30 s × 2ⁿ ≤ 1 h) |
| Polityka | `RoutePolicy` (klasa → kandydaci, termin pierwszego zdarzenia per klasa: głos 1,2 s, interaktywne 1,5 s, tło 5 s; obwód; `DuoConfig` Mówczyni + Myślicielka, `Tempo`), `RoutePolicy::defaults` (bez kluczy → wszystko lokalnie; z kluczem → rozmowa/kod/planowanie/wizja/streszczanie przez API, głos-szybka i embeddingi lokalnie), `with_toml` (`[router]`) |
| Traity | `Router` (`route`, `report`, `breaker_state`, `policy`), `BudgetGate` (budżet przez `cost_meter_contract::evaluate`), `RouterClock` |
| Zdarzenia | `router.decision`, `router.fallback`, `router.breaker.opened/closed`, `router.no_route`, `router.plan_window.exhausted` (`RouterEvent`, bez treści rozmowy) |

Feature `contract-tests`: spójność decyzji, przypięcie, otwarcie obwodu, pusta klasa — na `-impl` i `-fake`.
