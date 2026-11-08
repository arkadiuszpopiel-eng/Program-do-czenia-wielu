# compliance — SPEC (v0)

## Cel
Rejestr zgodności tras (`compliance-registry.json`): status trasy (zielona/szara/zabroniona), data weryfikacji, cytat źródła i archiwalna kopia regulaminu, wyłącznik trasy, tagi prywatności/jurysdykcji; nieświeży rejestr degraduje trasę do „szarej". Polityka sesji „prywatne" i deny-listy Jądra jako dane. Karty zgodności w UI (PLAN §1.3, §5.5).

## Fala i priorytet
F1: v0 (rejestr, tagi z katalogu, wyłącznik trasy, deny-listy jako dane) — **zrobione**. F4: v1 (karty zgodności w UI, archiwum regulaminów, przypięte wersje CLI, `providers[].deny_domains`). P0 (v0).

## Kontrakt (`compliance-contract` — źródło prawdy)
- `Registry::from_json` — format wersjonowany (`schema_version` ∈ `SUPPORTED_SCHEMA_VERSIONS = [1]`), walidacja: duplikaty, nieznany dostawca, `forbidden` + `enabled_by_default`, sufiks `.api` zarezerwowany. JSON Schema: `registry_schema()`.
- `RouteId`: trasy rejestru (`claude-code-cli`) + trasy API z katalogu `<provider>.api` (status z `compliance_status` katalogu; `unverified` = szara z ostrzeżeniem, domyślnie włączona).
- `effective_status(declared, verified_at, today, max_age_days)`: `dziś − verified_at > max_age_days` → co najwyżej `Grey` (`stale`); `Forbidden` zostaje.
- `RouteTable` (czysta logika wspólna `-impl`/`-fake`): widoki `RouteView`, tagi = suma rejestr ∪ katalog (nigdy nie zmniejsza ryzyka), wyłączniki.
- `route_allowed(route, SessionTag) -> Decision { allowed, reason: DecisionReason }`; kolejność: istnienie → `Forbidden` → wyłącznik → polityka sesji prywatnej (jurysdykcja `CN`; tag „może trenować": `cn-may-train`, `google-personal-may-train`; `unknown`/brak tagów = jak „może trenować") → `AllowedWithWarning` dla szarej.
- `trait Compliance`: `route`, `view(s)`, `effective_status`, `tags`, `route_allowed`, `set_enabled(id, on, ChangeOrigin)`, `deny_lists`, `is_denied_path`, `is_denied_domain`, `replace_deny_lists(&KernelAuthority, DenyLists)`.
- Deny-listy (`DenyLists`, dane): prefiksy (profile Chrome/Edge/Brave/Firefox/Opera, `Microsoft\Credentials|Vault|Protect`, `%USERPROFILE%\.docker\config.json`, `%APPDATA%\gh`), segmenty (`.claude`, `.claude.json`, `.codex`, `.gemini`, `.grok`, `.kimi`, `.agy`, `.ssh`, `.gnupg`, `.aws`, `.azure`, `.kube`, `.npmrc`, `.pypirc`, `.netrc`, `_netrc`, `.git-credentials`), domeny webowych UI dostawców (z subdomenami). Normalizacja ścieżek Windows: `%VAR%`, `$env:`, `~`, `\\?\`, `\\.\`, `\??\`, `UNC`, `\\localhost\c$`, separatory, wielkość liter, `.`/`..`, końcowe kropki/spacje, ADS, aliasy 8.3, `file:`; hostów: schemat, userinfo, port, `%XX`, kropki unikodowe, host z nie-ASCII → mapowanie IDNA/UTS 46 (`idna`, jak `url`/WebView2: `ⅽlaude.ai`, pełna szerokość → `claude.ai`; IDN → punycode; niepoprawny → brak hosta). Regresje przeglądu 2026-10: `tests/review.rs`.
- `KernelAuthority(())` z ukrytym konstruktorem `__broker_only()` — umowa do F3 (Broker). Segmentów `.claude`/`.codex` nie da się usunąć.
Zdarzenia: `compliance.route.enabled`, `compliance.route.disabled`, `compliance.route.stale` (przy starcie), `compliance.denylist.updated`. (`status_changed`, `cli_version.unknown` — F4.)

## Zależności
`core-bus-contract`, `core-registry-contract`. Dane: `docs/compliance/compliance-registry.json` (wbudowany jako domyślny), tagi katalogu jako `ProviderPolicyInput` (dostarcza kompozycja z `accounts-hub`).

## Niezmienniki
- Trasa `Forbidden` nie może być włączona (użytkownik ani Broker); agentki, Ulepszacz i moduły nie przestawiają wyłączników (`NotPermitted`).
- `verified_at + max_age_days < dziś` → status co najwyżej `Grey`; nieświeża trasa domyślnie wyłączona, jawne włączenie → ostrzeżenie.
- Tagi to suma źródeł; sesja prywatna nigdy nie trafia do CN / „może trenować" / `unknown`.
- Rejestr i deny-listy są polityką Jądra; zmiana deny-list tylko z `KernelAuthority`.

## Zdolności / uprawnienia
Brak.

## Izolacja
`inproc`, `always`.

## Budżet zasobów
RAM ≤ 1 MB; odczyt rejestru ≤ 5 ms; `route_allowed` bez I/O.

## Konfiguracja (klucze TOML)
`[compliance] registry = "compliance-registry.json"`, `stale_after_days` (domyślnie `max_age_days` z rejestru = 30), `[compliance.routes.<id>] enabled = true|false` (wyłącznik użytkownika; niepoprawne → `ignored_overrides`).

## Wkład do UI
Karty zgodności przy dostawcach/mostach (Ustawienia → Modele i dostawcy): status, data, cytat, link, wyłącznik; ostrzeżenie „szara trasa" przy dodawaniu konta; komunikaty z `DecisionReason` (Display po polsku).

## Testy akceptacyjne
- `ACC-F1-compliance-01`: statusy efektywne (świeży/nieświeży/wyłączony) — kontrakt + property-based na datach (`staleness_is_monotonic_in_age`).
- `ACC-F1-compliance-02` / F1-13: wyłączona trasa — 0 zgód w 100 próbach (`disabled_route_zero_calls`).
- Deny-listy: property-based obejścia (`..`, wielkość liter, ukośniki, prefiksy urządzeń, zmienne, kropki, ADS).
- `ACC-F4-compliance-03`: nieznana wersja CLI → trasa wyłączona (F4).

## Fake
`compliance-fake::FakeCompliance`: rejestr w pamięci, sterowana data (`set_today`, `advance_days`) i statusy (`set_status`), rejestr zapytań `route_allowed`; ta sama logika `RouteTable`.

## Otwarte pytania
- Podpis rejestru (minisign) i archiwum regulaminów — v1 (F4).
- Kanonizacja ścieżek przez OS (junction, symlink, prawdziwe 8.3) — `platform-windows-impl` przed dostępem do pliku.
- Tag konta Google (osobiste vs płatne/EOG) per konto zamiast sumy tagów dostawcy — SPEC v1.
