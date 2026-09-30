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
