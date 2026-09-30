# accounts-hub — SPEC (v0)

## Cel
Konta i klucze dostawców dodawane w dowolnym momencie, bez restartu i bez zmian w kodzie: deklaratywny katalog dostawców (`providers-catalog/*.toml`), kreator „Dodaj dostawcę / konto / klucz" (test połączenia, wykrycie modeli, przypisanie do klas zadań i głosu, limit kosztów), klucze wyłącznie w Windows Credential Manager (PLAN §5.6). Wykrywanie mostów CLI (tylko PATH + wersja) od F1; logowanie w ConPTY — F4.

## Fala i priorytet
F1 (katalog, kreator, Credential Manager, stany, import ze zmiennych, wykrywanie CLI) — **zrobione w v0**. F4: krok „mosty CLI" (logowanie). P0.

## Kontrakt (`accounts-hub-contract` — źródło prawdy)
- `ProviderCatalogEntry::from_toml` (pola `schema.json` + opcjonalne `env_vars`; `base_url`/`terms_url` `None` = `"TODO"`), `models` (wykryte), `pricing: PriceTable` (mikro-USD/Mtok z konfiguracji, `*` = domyślna), `policy_input()` → `compliance`.
- `Account { id, provider, label, created_at, last_test, state, secret: SecretName, base_url, assignments, cost_limit: Option<CostLimit>, source, models }`; `AccountState` = `Unconfigured | Active | Error { kind } | Disabled`; `provider_state()` (brak kont = `Unconfigured`).
- `SecretString` (zeroize, `Debug` = `***`, bez `Serialize`/`Display`); `trait SecretStore { put, get, delete, list }`.
- `Wizard` — maszyna stanów: `ChooseProvider → EnterKey → TestConnection → DiscoverModels → Assign → CostLimit → Confirm`; zabroniony dostawca / `oauth_cli` odrzucone; `finish` tylko po udanym teście bieżącego klucza.
- Porty: `ConnectionTester`, `ModelLister`, `EnvSource`, `CliProbe` (+ `detect_cli_bridges_with`, `parse_version`).
- `trait AccountsHub`: `catalog`, `provider`, `provider_state`, `accounts`, `account`, `set_pricing`, `add_account`, `test_account`, `rotate`, `remove`, `set_disabled`, `update_settings`, `import_from_env`, `resolve_secret(id, caller)` (tylko `providers-*`), `wizard_test`, `wizard_discover`, `wizard_finish`; `AccountsRepository` (metadane bez sekretów).
Zdarzenia (bez wartości kluczy): `accounts.key.added`, `accounts.key.removed`, `accounts.key.tested`, `accounts.key.rotated`, `accounts.state_changed`.

## Zależności
`compliance-contract` (tagi, status API, opcjonalnie trasa `<provider>.api` zabroniona), `core-bus-contract`, `core-registry-contract`. **Kierunek z `cost-meter`:** to `cost-meter-contract` zależy od `accounts-hub-contract` (ceny, `CostLimit`), nie odwrotnie. Test połączenia przez adaptery `providers-*` (kompozycja wstrzykuje `ConnectionTester`/`ModelLister`).

## Niezmienniki
- Wartość klucza istnieje tylko w `SecretStore` i jako `SecretString`; nigdy w metadanych, zdarzeniach, `Debug`, komunikatach błędów (redakcja) — testy szpiegowskie.
- Dodanie/rotacja/usunięcie działa bez restartu (zdarzenie → Router przelicza trasy); po usunięciu `resolve_secret` → `UnknownAccount`.
- Konto bez klucza w magazynie (np. metadane z innej maszyny) ma stan `Unconfigured`.
- Import ze środowiska tylko na życzenie i tylko ze zmiennych `env_vars` katalogu; bez zabronionych dostawców i duplikatów.
- Kod huba nie odwołuje się do katalogów poświadczeń CLI (test grep w źródłach); `--version` uruchamiane z wyczyszczonym środowiskiem i limitem czasu.

## Zdolności / uprawnienia
`secrets.read` / `secrets.write` (Credential Manager) — tylko ten moduł; `net.egress(host)` dla testu połączenia (w adapterze).

## Izolacja
`inproc`, `lazy`.

## Budżet zasobów
RAM ≤ 3 MB; test połączenia i wykrywanie modeli ≤ 10 s (timeout → `Error { Timeout }`); zero CPU w bezczynności.

## Konfiguracja (klucze TOML)
`[accounts.<account_id>] provider, label, assignments.*, cost_limit` (bez sekretów; plik `accounts.json` w `JsonFileRepository` do czasu `core-config-impl`), `[accounts] catalog_dir = "providers-catalog"`. Nazwy zmiennych do importu — pole `env_vars` w katalogu.

## Wkład do UI
Ustawienia → Modele i dostawcy → Hub kont i kluczy (makieta 12): lista, kreator (kroki `WizardStep`, ostrzeżenia `WizardWarning`), test, limity, usuń; onboarding „konta i klucze: dodaj teraz / pomiń".

## Testy akceptacyjne
- `ACC-F1-accounts-hub-01`: dodanie klucza (atrapa) bez restartu → konto `Active`, zdarzenie `accounts.key.added`.
- `ACC-F1-accounts-hub-02`: test szpiegowski — klucz nieobecny w metadanych, zdarzeniach, `Debug`, komunikatach.
- `ACC-F1-accounts-hub-03`: usunięcie → dostawca `Unconfigured`, 0 odczytów klucza po usunięciu.
- Katalog z repo: każdy plik przechodzi JSON Schema; tagi spójne z rejestrem zgodności.

## Fake
`accounts-hub-fake`: `FakeAccountsHub` (id `acc-N`, zegar sterowany, `set_state`, `secret_reads`), `MemorySecretStore`, `ScriptedConnectionTester`/`ScriptedModelLister` (kolejka albo prefiks klucza `sk-ok|sk-bad|sk-rate|sk-net|sk-slow`), `MapEnv`.

## Otwarte pytania
- Credential Manager przez `keyring-core` + `windows-native-keyring-store` w tym crate'cie vs przez `platform-windows-impl` (`SystemPort`) — do decyzji przy F1 platformy (ADR).
- Egzekwowanie `resolve_secret` tokenem zdolności Brokera (F3); aktualizacja katalogu z repo aktualizacji; import z `secrets.enc` (`transfer` v1).
