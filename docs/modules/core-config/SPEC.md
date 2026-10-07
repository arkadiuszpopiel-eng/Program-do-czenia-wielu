# core-config — SPEC (szkic v0)

## Cel
Konfiguracja warstwowa: pliki `%APPDATA%\Alfa\config\*.toml` (wspólne) + nakładka per maszyna `config/machine/<id>.toml`, walidowane JSON Schema z manifestów modułów, przeładowanie na żywo, historia w git z diffem i rollbackiem (PLAN §15, §3.5). Nie przechowuje sekretów.

## Fala i priorytet
F0 (kontrakt + fake). P0. Profile Prosty/Zaawansowany/Ekspert i edytor surowy — F1 (Ustawienia).

## Kontrakt (szkic Rust)
```rust
// core-config-contract — SZKIC
pub enum Scope { Global, Machine(MachineId), Session(SessionId), Agent(PersonaId) }
pub struct ConfigKey(pub String);              // np. "voice.tts.engine"
pub struct Setting { pub key: ConfigKey, pub value: toml::Value, pub scope: Scope,
                     pub default: toml::Value, pub schema: SchemaRef, pub kernel_policy: bool }
pub trait Config: Send + Sync {
    fn get<T: DeserializeOwned>(&self, key: &ConfigKey, scope: Scope) -> Result<T, ConfigError>;
    fn set(&self, key: &ConfigKey, value: toml::Value, scope: Scope, origin: Origin) -> Result<(), ConfigError>;
    fn watch(&self, prefix: &str) -> Subscription<ConfigChange>;
    fn history(&self, key: Option<&ConfigKey>) -> Vec<ConfigRevision>;
    fn rollback(&self, rev: RevisionId) -> Result<(), ConfigError>;
}
pub enum Origin { User, Module(ModuleId), Improver, Import, Broker }
```
Zdarzenia: `config.changed` (klucz, zakres, origin, diff), `config.reloaded`, `config.invalid` (plik nie przeszedł walidacji — zostaje poprzednia wartość), `config.rolled_back`.

## Zależności
`core-bus-contract`, `core-log-contract`; `platform-windows-contract` (ścieżki, obserwator plików).

## Niezmienniki
- Priorytet: sesja/agentka > maszyna > wspólna > domyślna z manifestu.
- Klucze oznaczone `kernel_policy = true` (polityki Jądra: uprawnienia, egress-allowlista, tagi prywatności, budżety, deny-listy, progi bramki) zmienia **tylko** `Origin::Broker`; inne `Origin` → `ConfigError::KernelPolicy` + zdarzenie Audytu.
- `Origin::Improver` może zmieniać tylko klucze R0 (PLAN §12.1) i tylko na wartości „zawężające" wg schematu.
- Sekrety nigdy w TOML (walidacja schematu odrzuca klucze `*_key`, `*_token`, `password`).
- Niepoprawny plik nie nadpisuje działającej konfiguracji; każda zmiana to commit w lokalnym repo git konfiguracji.
- Każde ustawienie ma opis, domyślną, zakres, reset (metadane z manifestu).

## Zdolności / uprawnienia
`fs.read/write(%APPDATA%\Alfa\config\**)` — przydzielane jądru; moduły zmieniają konfigurację przez kontrakt, nie przez pliki.

## Izolacja
`inproc`, `always`.

## Budżet zasobów
RAM ≤ 2 MB; wczytanie ≤ 30 ms; reakcja na zmianę pliku ≤ 200 ms.

## Konfiguracja (klucze TOML)
`[core.config] hot_reload = true`, `history = "git"`, `profile = "prosty" | "zaawansowany" | "ekspert"`. Identyfikator maszyny: `machine.id` (z `device-profile`).

## Wkład do UI
Ustawienia (drzewo + wyszukiwarka) buduje się z metadanych `Setting`; edytor surowy i historia zmian (Zaawansowane).

## Testy akceptacyjne
- `ACC-F0-core-config-01`: test kontraktowy — priorytety warstw, watch, rollback.
- `ACC-F0-core-config-02`: klucz `kernel_policy` odrzuca `Origin::User/Module/Improver` (100% przypadków).
- `ACC-F1-core-config-03`: plik z błędem składni podczas hot-reload → `config.invalid`, wartości bez zmian.

## Fake
`core-config-fake`: konfiguracja w pamięci z fixture'ami TOML, symulacja zmian plików i historii bez git.

## Otwarte pytania
- Repo git konfiguracji: `git2` vs własny prosty dziennik rewizji — do ustalenia w SPEC v1.
- Format „zawężająca zmiana" dla R0 (jak wyrazić w schemacie) — do ustalenia w SPEC v1 razem z `improver`.

## Zmiany po implementacji (F0: `core-config-impl`, `core-config-fake`)
- **Kontrakt (dodane):** `KERNEL_POLICY_PREFIX = "kernel"`, `ConfigKey::is_kernel_policy()`, `authorize(key, origin)` (wspólna reguła
  `-impl`/`-fake`) i `contract_tests` (feature). F0: polityka Jądra = prefiks `kernel.*` (flaga `kernel_policy` w schemacie — później).
- **Pliki:** `shared.toml` (Shared), `machine/<id>.toml` (Machine bieżącej maszyny; `id` = `[A-Za-z0-9_-]{1,64}`), `history.ndjson`.
  Nadpisania zakresów w tabelach `["@session".<id>]` / `["@agent".<id>]` (znak `@` nie występuje w `ConfigKey`). Priorytet:
  zakres(maszyna) > zakres(wspólna) > maszyna > wspólna > domyślna. Warstwa Default (z `default` schematów) i nakładka innej maszyny
  są tylko do odczytu (`Persist`). Wartości `null` i obiekty → `SchemaViolation` (klucze są liśćmi).
- **Schematy:** `register_schema(prefix, JSON Schema)` — walidacja wynikowego poddrzewa `prefix.*` przy `set` i `reload`
  (crate `jsonschema` 0.58.3 bez domyślnych funkcji: bez pobierania zdalnych `$ref`). Tryb `strict_keys` → `UnknownKey` poza schematami.
  Rejestracja nie waliduje wstecz wartości już zapisanych.
- **Historia:** zamiast repo git — append-only `history.ndjson` (`ts, key, scope, layer, old, new, origin, source=api|file`);
  rollback — później (otwarte pytanie „git2 vs dziennik” rozstrzygnięte na razie na dziennik).
- **Przeładowanie:** jawne `reload()` i `watch_files` (`notify` 8.2.0, debounce domyślnie 100 ms). Plik niepoprawny (składnia, sekret,
  zmiana `kernel.*` z pominięciem Brokera, naruszenie schematu) → wartości bez zmian, `config.invalid`, a zapis do tego pliku jest
  wstrzymany do naprawy (by nie nadpisać ręcznej edycji). Przy otwarciu niepoprawny plik nie blokuje startu (`load_problems()`).
- **Zdarzenia:** `config.changed` (tylko przy zmianie wartości wynikowej), `config.reloaded`, `config.invalid`,
  `config.kernel_policy_rejected` (poziom Warn; zdarzenie Audytu zapisze Broker w F3). `watch` dostaje zmiany zakresu, w którym zapisano.
- **Nie w F0:** `Origin::Improver` tylko R0/zawężająco, rollback, profile, zdarzenie `config.rolled_back`.
