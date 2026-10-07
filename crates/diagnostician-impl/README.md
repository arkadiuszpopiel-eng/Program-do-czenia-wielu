# diagnostician-impl

Usługa Diagnosty (`docs/modules/diagnostician/SPEC.md`, PLAN §12.2):

- `DiagnosticianService` — rdzeń `DiagnosticianCore` (kontrakt) + `ServiceHost`: dziennik napraw `FileJournal`
  (NDJSON append-only, `fsync` po wpisie; uszkodzona ostatnia linia → `journal_problems`, nie blokuje startu),
  odtworzenie stanu i **odzysk** napraw przerwanych między wykonaniem a weryfikacją (cofane przy otwarciu),
  zdarzenia `diagnostician.*` na magistralę i bufor ostatnich zdarzeń.
- Po `start`: subskrypcja magistrali → `Signal::from_event` (`registry.module.*`, `watchdog.*`, `config.invalid`,
  `diagnostics.symptom`) → `ingest`; skan co `DEFAULT_SCAN_EVERY_MS` (30 s, `with_scan_interval`).
- Adaptery portów dla `app-*`: `PortsEnv` (`RepairEnv` z wąskich portów: `ConfigStore` z porównaj-i-zamień
  i `Origin::Module("diagnostician")`, `ConfigHistory` watchdoga, `FilePort`, `ModuleRestarter`, `EntryStore`,
  `DownloadQueue`, `HealthProbe`), `LocalFiles` (pliki tylko w dozwolonych korzeniach, bez `..` i ucieczki
  dowiązaniem, bez nadpisywania, usuwanie wyłącznie kopii identycznej SHA-256), `DirContext` (`RepairContext`:
  kopie, kwarantanna, archiwum, korzenie Jądra, wolne porty przez `bind` na 127.0.0.1).
- `module.toml` (`always`, `inproc`).

Testy: kontraktowe, chaosowe na całym katalogu (24 awarie, F8-01/F8-06) i zgodność z `evals/F8/chaos/catalog.json`
(`tests/contract.rs`), dziennik i odzysk (`tests/journal.rs`), adaptery (`tests/ports.rs`), magistrala i cykliczny
skan (`tests/service.rs`).
