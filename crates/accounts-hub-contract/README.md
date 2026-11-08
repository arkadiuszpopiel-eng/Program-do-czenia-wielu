# accounts-hub-contract

Kontrakt huba kont i kluczy (docs/modules/accounts-hub/SPEC.md, PLAN §5.6).

- **Katalog**: `ProviderCatalogEntry::from_toml(text, stem)` — model wpisu `providers-catalog/*.toml`
  (dokładnie pola `schema.json`, `deny_unknown_fields`) + reguły semantyczne (`id` = nazwa pliku,
  pusty cennik, `https://` albo `TODO`, nazwy `env_vars`); `models` (wykryte) i `pricing`
  (`PriceTable`, mikro-USD/Mtok, z konfiguracji) uzupełnia hub. `policy_input()` → dane dla `compliance`.
- **Konta**: `Account` (id, dostawca, etykieta, utworzono, ostatni test, stan, uchwyt sekretu,
  przypisania, limit kosztów `CostLimit` — wyłączalny), `AccountState` = nieskonfigurowany / aktywny /
  błąd / wyłączony, `provider_state()`, `TestOutcome::next_state()`.
- **Sekrety**: `SecretString` (zeroize, `Debug` = `***`, bez `Display`/`Serialize`/`PartialEq`,
  `ct_eq`, `redact_in`) i trait `SecretStore` (`put/get/delete/list`, nazwy `SecretName` bez prefiksu).
- **Kreator** `Wizard` — czysta maszyna stanów: dostawca → klucz → test → modele → przypisania → limit
  → potwierdzenie; `finish` tylko po udanym teście bieżącego klucza (property test).
- **Porty**: `ConnectionTester`, `ModelLister`, `EnvSource`, `CliProbe` + `detect_cli_bridges_with`,
  `parse_version` (mosty CLI: tylko PATH i `--version`).
- Trait `AccountsHub`, `AccountsRepository`, zdarzenia `accounts.key.added|removed|tested|rotated`,
  `accounts.state_changed` (bez wartości kluczy).
- Feature `contract-tests`: `contract_tests::run_all` + fixture katalogu i konwencja kluczy skryptu.

Zależy od `compliance-contract` (tagi, status API) i `core-bus-contract`; zewnętrznie od `zeroize`.
