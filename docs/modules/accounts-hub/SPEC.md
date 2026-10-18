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
- Egzekwowanie `resolve_secret` tokenem zdolności Brokera (F3); aktualizacja katalogu z repo aktualizacji; ~~import z `secrets.enc` (`transfer` v1)~~ — odrzucone (CX-a: sekrety nigdy w `.alfa`).

## Poprawki po recenzji PR #1 (2026-10-04)
- **Q-5 (zrobione):** zmiany kont (`add_account`, `update_settings`, `rotate`, `test_account`, `set_disabled`, kreator,
  import z env, `remove`) trafiają do pamięci **dopiero po udanym zapisie** `accounts.json`
  (`AccountsHubService::update_accounts`, blokada zapisu na czas zapisu). Błąd zapisu → pamięć zgodna z plikiem;
  nieudane dodanie usuwa świeżo zapisany klucz (bez osieroconych wpisów w Credential Manager); `remove` najpierw usuwa
  klucz (błąd → nic się nie zmienia), potem metadane. Test: `accounts-hub-impl/tests/persist_failure.rs`.
- Znane ograniczenie (usunięte w 0.0.7 — patrz niżej): `rotate` zapisuje nowy klucz przed metadanymi — przy błędzie zapisu klucz jest już nowy, a stan
  konta (ostatni test) stary; ponowna rotacja/test wyrównuje.

## Diagnoza i wymiana klucza (wersja 0.0.7, 2026-10-09)
- `TestOutcome` = `Ok | InvalidKey | Restricted | NoCredit | Refused | RateLimited | Network | Timeout | Unsupported`;
  klucz „działa” przy `Ok`, `Restricted`, `RateLimited`. Raport niesie `ProbeDetail { status, code, message }` — odpowiedź
  dostawcy po maskowaniu wszystkiego, co wygląda jak klucz (`app-modules/src/probe.rs::scrub`), do 400 znaków.
- `SecretString::from_input` zdejmuje to, co dokleja schowek (cudzysłowy, `NAZWA_KEY=`, `export`/`set`/`$env:`,
  `Authorization: Bearer`, `,;`, znaki niewidoczne, znaki nowej linii) i odrzuca tekst, który potem nie jest ciągiem
  drukowalnych znaków ASCII bez spacji. Te same przypadki sprawdza interfejs (`tests/fixtures/key-inputs.json`).
- `try_rotate(id, secret)`: najpierw test kandydata; do magazynu trafia tylko klucz, który dostawca rozpoznał
  (`Ok`, `Restricted`, `NoCredit`) albo którego nie da się sprawdzić (`Unsupported`) — `TestOutcome::accepts_new_key`.
  Limit zapytań, sieć i odmowa zostawiają poprzedni klucz. Przy błędzie zapisu metadanych wraca poprzedni klucz
  (zastępuje „znane ograniczenie” z sekcji wyżej).
- Dostawca z publiczną listą modeli (OpenRouter) daje `Unsupported`, nie `Ok` — udany odczyt nic nie mówi o kluczu.
- `validate_base_url`: host wyłuskany z adresu; dane logowania (`@`) odrzucane; `http://` tylko dla hosta lokalnego.
- `set_disabled(id, false)` przywraca stan z ostatniego testu (konto z odrzuconym kluczem nie staje się aktywne).
- `set_manual_model(id, model)`: model podany ręcznie dla konta bez listy modeli od dostawcy; udany, niepusty odczyt
  listy go zastępuje, pusta lista niczego nie kasuje.
- `find_by_secret(provider, base_url, secret)`: ten sam klucz u tego samego dostawcy pod tym samym adresem = odmowa
  z nazwą istniejącego konta.
- Wynik testu jest scalany z bieżącym wpisem konta (zmiany przypisań i limitu z czasu testu nie giną).
- Testy całej ścieżki: `app-core/tests/accounts_keys.rs`, `accounts_rotate.rs` (lokalny serwer udający dostawcę).

## Przeznaczenie modeli i kolejki (wersja 0.0.8, 2026-10-09)
- `model_use(id)` rozpoznaje po nazwie, do czego służy model (`Chat`, `Reasoning`, `Embedding`, `Speech`,
  `Transcription`, `Realtime`, `Image`, `Video`, `Moderation`, `Other`); `best_chat_model` wybiera model domyślny
  rozmowy — nigdy pierwszy z brzegu (błąd `tts-1` z laptopa).
- `Account.queues: klasa zadań → modele` (najwyżej `MAX_QUEUE = 10`); `set_model_queue(id, klasa, modele)` sprawdza
  limit, powtórzenia i dopasowanie do klasy (`validate_queue`); pusta lista = wybór automatyczny.
- `suggest_queue(modele, klasa, strategia)` — propozycja (`Quality` / `Balanced` / `Economy`), bez przypiętych migawek.
- Router dostaje kolejki przez `RouterCore::set_provider_queues` i w polityce automatycznej wystawia modele
  dostawcy po kolei; błąd „model nie istnieje / brak dostępu” przełącza na następny (`router-impl/src/fallback.rs`).
- Klasa zadań tury pochodzi z „klasy modelu” głównej roli agentki (`BrainRequest.task`).
- Testy: `app-core/tests/accounts_models.rs`.
- **0.0.20 (C2.14):** `Account.key_saved_at` — chwila zapisania obecnego klucza (`add_account`, `rotate`, `try_rotate`);
  brak pola w starych danych = `created_at`. Okresowe sprawdzanie kont i przypomnienie o wymianie żyją w kompozycji
  (`app-core/src/parts/key_watch.rs`) i w interfejsie — hub tylko przechowuje datę i wykonuje test.

## Konta firmowe (wersja 0.0.64, C2.12)
- `Account.enterprise: EnterpriseSettings` (pole pomijane, gdy puste; stare dane wczytują się bez zmian) i to samo pole
  w `NewAccount` i `ConnectionRequest` — test i lista modeli idą z ustawieniami konta.
- OpenAI (`openai`): opcjonalne `organization` (`org-…`) i `project` (`proj_…`, dopuszczalnie `proj-…`) → nagłówki
  `OpenAI-Organization` i `OpenAI-Project` w każdym żądaniu konta. To nie sekrety, ale `Debug` pokazuje je skrócone
  (`mask_id`: `org-…wxyz`), a silnik HTTP oznacza te nagłówki jako wrażliwe.
- Azure OpenAI (`azure-openai`, wpis katalogu bez adresu): `base_url` = adres zasobu `https://<zasób>.openai.azure.com`
  (także `*.cognitiveservices.azure.com`; host lokalny dla bramek i testów), `api_version` (domyślnie `2024-10-21`),
  `deployments: [{ name, model? }]` — model Alfy → wdrożenie (1–32, bez powtórzeń modelu). Klucz jak każdy inny —
  wyłącznie w Menedżerze poświadczeń.
- `EnterpriseSettings::checked(dostawca, adres)` oczyszcza (odstępy, puste pola) i odrzuca to, co nie pasuje do
  dostawcy (`AccountsError::InvalidInput`, w kreatorze `WizardError::InvalidEnterprise`). Te same reguły w interfejsie:
  `logic/enterprise.ts`, wspólna tabela przypadków `accounts-hub-contract/tests/fixtures/enterprise-cases.json`.
- Kontrakt: `contract_tests::enterprise_settings_only_where_they_apply` (impl i fake).
