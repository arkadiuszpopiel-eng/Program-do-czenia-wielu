# accounts-hub-impl

Hub kont i kluczy. `AccountsHubService` (budowany `AccountsHubService::builder(...)`) implementuje
`AccountsHub` i `Module`:

- katalog: `CatalogValidator` — TOML → JSON → walidacja `providers-catalog/schema.json` (wbudowany,
  `jsonschema` bez pobierania zdalnych referencji) → model; `load_dir` nie blokuje pozostałych wpisów
  przy błędzie jednego;
- klucze wyłącznie w `SecretStore`: na Windows `CredentialManagerStore` (`cfg(windows)`,
  `keyring-core` + `windows-native-keyring-store`, nazwy `Alfa/<nazwa>`, trwałość `Local`,
  bez `unsafe` w kodzie Alfy); metadane kont bez sekretów w `JsonFileRepository` (zapis atomowy);
- test połączenia i wykrywanie modeli przez porty z limitem czasu (domyślnie 10 s), komunikaty
  redagowane (`SecretString::redact_in`); stan konta z wyniku testu;
- rotacja, usunięcie, wyłączenie bez restartu; zdarzenia `accounts.*` bez wartości kluczy;
- import kluczy na życzenie (`import_from_env`) wyłącznie ze zmiennych z `env_vars` katalogu
  (`ProcessEnv`), z pominięciem dostawców zabronionych i duplikatów;
- `resolve_secret(id, caller)` tylko dla modułów `providers-*` (pełne tokeny zdolności — Broker, F3);
- opcjonalnie `Compliance`: trasa `<provider>.api` zabroniona → dodanie konta odrzucone;
- `detect_cli_bridges()` — `claude`/`codex` w PATH (PATHEXT na Windows) + `--version` z limitem
  czasu i wyczyszczonym środowiskiem (bez kluczy API). Test `no_cli_credential_paths_in_sources`
  sprawdza, że kod crate'ów accounts-hub nie odwołuje się do katalogów poświadczeń CLI.
