# compliance-contract

Kontrakt modułu zgodności v0 (docs/modules/compliance/SPEC.md, PLAN §1.3, §5.5).

- **Rejestr** `compliance-registry.json`: typy `Registry`/`RegistryRoute`/`RegistryProvider`, parser
  z kontrolą wersji formatu (`SUPPORTED_SCHEMA_VERSIONS = [1]`) i walidacją (duplikaty, nieznany
  dostawca, zabroniona trasa włączona domyślnie); JSON Schema: `registry_schema()`.
- **Statusy**: `RouteStatus` (green/gray/forbidden), `effective_status` — wpis starszy niż
  `max_age_days` (domyślnie 30, konfigurowalne) degraduje się do „szarej”, zabroniona zostaje zabroniona.
- **Tagi**: `PrivacyTag`, `Jurisdiction` (`CN`, `SG|EU`, `unknown`), `RouteTags` (suma rejestr ∪ katalog).
- **Tabela tras** `RouteTable`: trasy z rejestru + trasy API z katalogu (`<provider>.api`),
  wyłączniki (`ChangeOrigin::User`/`Broker`; agentki i Ulepszacz — `NotPermitted`).
- **Decyzja** `decide`/`route_allowed(route, SessionTag) -> Decision { allowed, reason }`:
  istnienie → zabroniona → wyłącznik → polityka sesji prywatnej (CN, „może trenować”, `unknown`)
  → ostrzeżenie dla szarej.
- **Deny-listy** (`deny`): `DenyLists` jako dane, `DenyChecker::is_denied_path/is_denied_domain`
  z normalizacją ścieżek Windows (`%VAR%`, `$env:`, `~`, `\\?\`, `\\.\`, `\??\`, UNC `\\localhost\c$`,
  separatory, wielkość liter, `.`/`..`, końcowe kropki/spacje, ADS, aliasy 8.3, URL `file:`)
  i hostów (schemat, userinfo, port, `%XX`, kropki unikodowe, subdomeny). Zmiana list wymaga
  `KernelAuthority` (konstruktor tylko dla Brokera; pełne egzekwowanie w F3) i nie może usunąć
  segmentów `.claude`/`.codex`.
- Trait `Compliance` + zdarzenia `compliance.route.enabled|disabled|stale`, `compliance.denylist.updated`.
- Feature `contract-tests`: `contract_tests::run_all` (fixture rejestru, daty świeża/nieświeża).

Czysta logika jest w kontrakcie celowo: `-impl` i `-fake` liczą decyzje tym samym kodem.
