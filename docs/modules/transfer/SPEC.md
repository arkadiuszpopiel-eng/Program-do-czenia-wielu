# transfer — SPEC (szkic v0)

## Cel
Import/eksport paczek `.alfa` między maszynami właściciela (bez synchronizacji) i kopie zapasowe jako zaplanowany eksport: wybór zakresu, manifest z sumami, opcjonalne szyfrowanie hasłem, dry-run, tryby dodaj/scal/zastąp, kolizje id, upcastery, snapshot przed importem, rollback (PLAN §15.1; format w `docs/formats/alfa-package.md`).

## Fala i priorytet
F1: v1 P0-lite (konfiguracja wspólna + agentki/obsady + wybrane sesje). F7: pełny (pamięć, umiejętności, artefakty, kopie zapasowe z harmonogramem i rotacją). P0.

## Kontrakt (szkic Rust)
```rust
// transfer-contract — SZKIC
pub struct ExportScope { pub config_common: bool, pub personas: bool, pub casts: bool, pub rules: bool, pub skills: bool,
    pub sessions: Vec<SessionId>, pub memory: Vec<MemoryScope>, pub artifacts: bool, pub logs: bool, pub config_machine: bool }
pub struct Manifest { pub schema_version: Version, pub app_version: Version, pub kind: PackageKind /* Export | Backup | Secrets */,
    pub created_at: Timestamp, pub source_machine: MachineInfo, pub scope: ScopeSummary, pub content: Vec<ContentEntry>, pub content_sha256: Hash,
    pub encryption: Option<EncryptionHeader> }
pub enum ImportMode { Add, Merge, Replace }
pub struct DryRunReport { pub items: Vec<ItemDiff /* New | Same | Changed | Collision { id } */>, pub warnings: Vec<Warning>, pub migrations: Vec<UpcastStep> }
pub trait Transfer: Send + Sync {
    fn export(&self, scope: ExportScope, dest: &Path, password: Option<Secret>, kind: PackageKind) -> Result<Manifest>;
    fn inspect(&self, pkg: &Path, password: Option<Secret>) -> Result<(Manifest, DryRunReport)>;
    fn import(&self, pkg: &Path, modes: ModeMap, resolutions: CollisionMap, password: Option<Secret>) -> Result<ImportReport>;
    fn rollback(&self, snapshot: SnapshotId) -> Result<()>;
}
```
Zdarzenia: `transfer.export.started/completed`, `transfer.import.dry_run`, `transfer.import.snapshot_created`, `transfer.import.completed` (Audyt), `transfer.rolled_back`, `transfer.backup.scheduled/completed`.

## Zależności
`core-bus/config/log-contract`, `sessions-contract`, `memory-contract` (F7), `artifacts-contract`, `personas-contract`, `accounts-hub-contract` (eksport sekretów — osobna operacja), `device-profile-contract` (id maszyny, `hw_class`), `platform-windows-contract` (pliki, Credential Manager).

## Niezmienniki
- Klucze i sekrety nigdy w zwykłej paczce (test szpiegowski na pełnym zakresie); eksport sekretów tylko jako `PackageKind::Secrets`, zawsze szyfrowany.
- Import zawsze poprzedzony dry-run i automatycznym snapshotem dotkniętych elementów; rollback jednym kliknięciem.
- Zapis transakcyjny per element: element nietknięty albo w całości zaimportowany.
- Ścieżki w paczce względne, bez `..` (odrzucenie zip-slip); sumy kontrolne weryfikowane przed zapisem.
- Paczka o nowszym `schema_version` odrzucana z komunikatem; starsza migrowana upcasterami (testy migracji na fixture'ach każdej wersji).
- Kopia zapasowa = ten sam kod eksportu; harmonogram nie startuje na baterii ani przy pełnym ekranie.

## Zdolności / uprawnienia
`fs.read/write` na katalogu docelowym wybranym przez użytkownika (jako Ty, z okna dialogowego), `secrets.read` tylko dla eksportu sekretów (po potwierdzeniu w Broker-UI od F3).

## Izolacja
`inproc`, `on-demand`.

## Budżet zasobów
Eksport 100 MB ≤ 30 s na baseline; RAM ≤ 50 MB (strumieniowo, bez ładowania paczki do pamięci); UI nie blokowane (postęp + anulowanie).

## Konfiguracja (klucze TOML)
`[transfer] snapshots_keep = 5`, `snapshots_dir = "%LOCALAPPDATA%\\Alfa\\snapshots"`, `[transfer.backup] enabled = false`, `dir`, `schedule = "0 3 * * *"`, `scope = [...]`, `rotation = { daily = 7, weekly = 4 }`, `password_in_credential_manager = false` (F7).

## Wkład do UI
Ustawienia → Import i eksport (makieta 13): kreator zakresu, dry-run z różnicami, tryby, kolizje, rollback; „eksport sesji do `.alfa`" w panelu Sesje; `/eksport`; krok onboardingu „import paczki z innej maszyny"; głos („Beta, wyeksportuj sesję X").

## Testy akceptacyjne
- `ACC-F1-transfer-01`: round-trip `.alfa` (config + sesje) desktop ↔ laptop bez utraty danych (runnery).
- `ACC-F1-transfer-02`: test szpiegowski — 0 sekretów w paczce pełnego zakresu.
- `ACC-F1-transfer-03`: import przerwany w połowie → stan spójny, snapshot pozwala na rollback (property-based na punktach przerwania).
- `ACC-F7-transfer-04`: test przywracania w CI — kopia → czysty profil → import → 0 różnic.

## Fake
`transfer-fake`: paczki w pamięci/tmp, skryptowane kolizje i błędy I/O, wirtualny zegar dla harmonogramu.

## Otwarte pytania
- Algorytm szyfrowania (propozycja: age / XChaCha20-Poly1305 + Argon2id) — ADR w F1.
- Podpis kopii zapasowych minisign; polityka sesji „prywatne"/`tainted` przy eksporcie — do ustalenia w SPEC v1.

## Zmiany po implementacji (F1, `transfer-contract/-impl/-fake`, 2026-10-01)
- **Kontrakt synchroniczny** (pliki, SQLite): `Transfer { export, export_secrets, inspect, import, snapshots, rollback,
  backup }`; `inspect(pkg, &ImportOptions)` = dry-run (tryby i rozstrzygnięcia wpływają na plan). Hasło = `SecretString`
  (`accounts-hub-contract`), min. 8 znaków. Anulowanie: `CancelToken` sprawdzany między elementami (postęp — później).
- **Silnik w kontrakcie** (`engine`): eksport z portów, plan (dry-run), snapshot, zapis per element, rollback — wspólny dla
  `-impl` i `-fake` (jak logika deterministyczna w `personas-contract`). Porty: `sessions-contract::Sessions`,
  `DocumentStore` per kategoria (konfiguracja, agentki, obsady, reguły, umiejętności, pamięć, artefakty, logi —
  adaptery podpina kompozycja buildu; `-impl` daje `DirDocumentStore`), `SecretStore`, `Clock`, `IdSource`.
  Persony jako **nieprzezroczysty dokument JSON** (`personas/*.json`, np. `PersonasExport`) — bez zależności od
  rozwijanego równolegle `personas-contract`.
- **Kontener**: ZIP (crate `zip` 8.6, tylko deflate), manifest pierwszy i nieskompresowany; zapis atomowy (plik tymczasowy
  + `rename`). Limity (`Limits`): wpisy 200 000, wpis 2 GiB, łącznie 16 GiB, manifest 64 MiB, stopień kompresji 1000
  (wpisy > 1 MiB). Reguła ścieżek rozszerzona o realia Windows (`\`, `:`, UNC, `CON`/`NUL`…, kropka/spacja na końcu).
- **Szyfrowanie (rozstrzygnięte)**: `ALFAENC1` + nagłówek JSON (AAD) + **XChaCha20-Poly1305 STREAM** (fragmenty 64 KiB,
  nonce prefiks 19 B ‖ licznik BE32 ‖ flaga ostatniego) — crate'y RustCrypto `chacha20poly1305` 0.10 i `argon2` 0.5;
  klucz **Argon2id** m = 46 MiB, t = 2, p = 1 (w budżecie RAM 50 MB), sól 16 B; odczyt nagłówka obcej paczki ograniczony
  (m ≤ 256 MiB, t ≤ 16, p ≤ 8). Odczyt losowy szyfrogramu — bez jawnej kopii na dysku.
- **Sekrety**: zwykła paczka przechodzi przez `SecretGuard` (dokładne wartości z `SecretStore` ≥ 8 znaków, wzorce
  `RegexRedactor`, wartości pod kluczami `api_key`/`token`/`secret`/`password`…; redakcja na poziomie wartości
  TOML/JSON/NDJSON), a każdy wpis jest skanowany przed zapisem — trafienie przerywa eksport (`SecretDetected`). Liczba
  redakcji w manifeście (`redactions`) i ostrzeżeniach. Eksport sekretów: paczka `secrets` (cała szyfrowana hasłem,
  `secrets.json` w środku); import wymaga `allow_secrets`. Sekrety `transfer/*` (klucz snapshotów) nie są eksportowane.
- **Sesje prywatne/`local_only`**: pomijane bez `include_private`; z nim — wyłącznie w paczce szyfrowanej. `tainted`
  przenoszony (tylko rośnie przy scalaniu).
- **Import**: stany `new/same/changed/collision` (kolizja = ta sama sesja z rozbieżną historią); tryby per kategoria;
  rozstrzygnięcia `merge/replace/copy/skip`; scalanie sesji = suma drzew (ta sama tura = ten sam rodzic + treść + czas),
  nowe tury dostają kolejne `id` i gałęzie jak przy `fork_from`; kopia = nowe `id` (`id_map`), tytuł „(import z …)”,
  unikalny katalog roboczy. Konfiguracja: klucze `kernel.*` usuwane (ostrzeżenie), nakładka maszyny tylko przy
  `include_machine_overlay` (ostrzeżenie przy innej `hw_class`; nazwa mapowana na `<id tej maszyny>.toml`).
- **Snapshot** = paczka `snapshot` (szyfrowana kluczem maszyny z Credential Managera; bez klucza — ostrzeżenie, a import
  sekretów przerwany) z wersjami sprzed importu i `rollback.json` (elementy utworzone przez import). Zastąpienie sesji
  przy błędzie przywraca wersję sprzed zapisu. Rotacja 5 snapshotów.
- **Kopie zapasowe**: `backup` = ten sam eksport (`kind = backup`) do `alfa-backup-<RRRRMMDD-GGMMSS-mmm>.alfa` + rotacja
  N ostatnich (obce pliki nietknięte). Harmonogram: `BackupSchedule::is_due` (interwał, nie na baterii, nie przy pełnym
  ekranie) — uruchamianie z harmonogramu w F7.
- Zdarzenia: także `transfer.export.started`; ładunki bez pełnych ścieżek (nazwa pliku), bez treści i sekretów.
- **Do zrobienia**: postęp operacji (zdarzenia z procentem), strumieniowy `DocumentStore::read` dla dużych artefaktów,
  załączniki sesji (`attachments/`), podpis minisign kopii zapasowych, harmonogram kopii (F7), MIME `.alfa`.
