# router-impl

Router (docs/modules/router/SPEC.md), manifest `module.toml` (`router`, `inproc`, `always`).

- `RouterCore` (`Router`): rejestr dostawców (`register`/`unregister` bez restartu — nowy klucz = nowa
  trasa), polityka automatyczna z zarejestrowanych dostawców (lokalny + API ze skonfigurowanym kluczem),
  jawna (`set_policy`) albo z nadpisaniami TOML (`set_overrides_toml`). Ocena kandydata (bez I/O):
  rejestr → klucz (`Unconfigured` niewidoczny, 401 → `AuthFailed`) → zgodność `route_allowed` +
  jurysdykcja z tagów rejestru + `check_privacy` (tylko API) → możliwości modelu → obwód → okno 429 →
  TTFT → budżet (`BudgetGate`, tylko API). Obwody i okna per dostawca, zdarzenia `router.*`.
- `RoutedProvider` — Router jako `ModelProvider` (dekorator) dla klasy: `model = "auto"` lub przypięcie
  `dostawca:model`; **fallback** przy `should_fallback()` przed pierwszą treścią albo po terminie
  pierwszego zdarzenia klasy — ta sama historia na kolejnym celu (0 utraconych wiadomości), jedno
  `Started` od celu, który odpowiada; błąd po częściowym wyjściu nie jest maskowany; pochodzenie bloków
  myślenia (`dostawca:model`) przywracane przed wysłaniem; osadzenia z fallbackiem.
- `CostMeterGate` — `BudgetGate` przez `cost_meter_contract::evaluate` (wydatki miesiąca, tła, dostawcy; kurs).
- `RouterModule` — moduł rejestru, zdarzenia na magistralę w kolejności.

Testy: zestaw kontraktowy; ACC-F1-router-01 (property-based, 500 losowych konfiguracji — decyzja = filtr
referencyjny); ACC-F1-router-02/F1-04 (50 prób 5xx/timeout na `providers-fake`: przełączenie ≤ 2 s, ta
sama historia; pomiar w czasie rzeczywistym); ACC-F1-router-03 (trasa wyłączona: 0 wywołań w 100 próbach);
prywatność/jurysdykcja, polityka domyślna (profil A → klucz dodany), budżet, TTFT, obwód half-open, okno 429,
anulowanie ≤ 100 ms, pochodzenie myślenia.
