# transfer-contract

Kontrakt modułu `transfer` (docs/modules/transfer/SPEC.md, format: docs/formats/alfa-package.md).

- **Trait `Transfer`**: `export` (sekrety nigdy; eksportu sekretów nie ma — CX-a), `inspect` (dry-run), `import`
  (dry-run → snapshot → zapis per element), `snapshots`, `rollback`, `backup` (eksport `backup` + rotacja).
- **Typy**: `ExportScope` (domyślny zakres z PLAN §15.1), `ImportOptions` (tryby dodaj/scal/zastąp per kategoria,
  rozstrzygnięcia kolizji: scal/zastąp/kopia/pomiń), `Manifest` + `Limits` (zip-bomb), raporty (`DryRunReport`, `ImportReport`…).
- **Ścieżki**: `validate_entry_path` — bez `..`, `\`, `:` (C:, strumienie NTFS), UNC, nazw zarezerwowanych Windows,
  segmentów z kropką/spacją na końcu; property-test w `transfer-impl`.
- **Format przenośny sesji** (`portable`): `sessions/<id>/session.json` + `turns.ndjson` (drzewo tur z gałęziami,
  wersja rekordu w każdej linii), katalog roboczy względny (bez nazwy konta Windows). **Upcastery** (`migrate`): v0 → v1.
- **Strażnik sekretów** (`SecretGuard`): dokładne wartości z `SecretStore` + wzorce `core_log_contract::RegexRedactor`
  + klucze typu `api_key/token/password`; redakcja na poziomie wartości (TOML/JSON/NDJSON) i skan końcowy surowych bajtów.
- **Scalanie dokumentów** (`docmerge`): TOML/JSON klucz po kluczu (paczka wygrywa), NDJSON suma z deduplikacją;
  klucze `kernel.*` nigdy nie są importowane (tylko Broker).
- **Silnik** (`engine::Engine`): eksport z portów, plan, snapshot, zapis, rollback — wspólny dla `-impl` i `-fake`.
  Porty: `sessions-contract::Sessions`, `DocumentStore` per kategoria, `accounts-hub-contract::SecretStore`, `Clock`, `IdSource`.
- Feature `contract-tests`: `contract_tests::run_all(factory)` (12 przypadków: round-trip, dry-run, tryby i kolizje,
  snapshot+rollback, test szpiegowski, szyfrowanie, brak eksportu sekretów, sesje prywatne, `kernel.*` i nakładka, kopie z rotacją…).
