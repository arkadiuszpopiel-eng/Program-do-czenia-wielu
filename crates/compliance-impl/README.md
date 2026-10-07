# compliance-impl

Moduł zgodności v0. `ComplianceService` implementuje `Compliance` i `Module`:

- rejestr domyślny wbudowany z `docs/compliance/compliance-registry.json` (`DEFAULT_REGISTRY_JSON`,
  `default_registry()`), alternatywnie `load_registry_file(path)`;
- tagi z katalogu dostawców przekazywane przy budowie (`ProviderPolicyInput`, dostarcza je kompozycja
  z `accounts-hub`), łączone z tagami rejestru (suma — nigdy nie zmniejsza ryzyka);
- konfiguracja `ComplianceConfig`: próg nieświeżości (`max_age_days`, domyślnie z rejestru = 30),
  wyłączniki użytkownika (niepoprawne → `ignored_overrides()`), deny-listy, zmienne ścieżek
  (`with_process_env`: `USERPROFILE`, `LOCALAPPDATA`, `APPDATA`, `SYSTEMDRIVE`);
- data „dziś” przez trait `Today` (`SystemToday` — data lokalna, `FixedToday` — testy);
- zdarzenia: `compliance.route.stale` przy starcie dla tras zdegradowanych, `compliance.route.enabled|disabled`
  przy zmianie wyłącznika, `compliance.denylist.updated` po podmianie list przez Brokera;
- `health()`: `Degraded`, gdy któraś trasa wymaga ponownej weryfikacji.

Zależności produkcyjne: `compliance-contract`, `core-bus-contract`, `core-registry-contract`.
Testy: współdzielony kontrakt, rejestr z repo (stan na 30.09.2026 i po terminie), cykl życia,
zdarzenia, wersjonowanie formatu, property-based na datach.
