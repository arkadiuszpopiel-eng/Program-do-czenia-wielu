# diagnostician — SPEC (v1: kontrakt zaimplementowany)

## Cel
Diagnosta (PLAN §12.2): zbiera sygnały (stan modułów z `core-registry`, symptomy z dziennika Diagnostyka `core-log`, restarty i safe-mode watchdoga, budżety RAM/CPU, wolne miejsce), klasyfikuje awarie wg katalogu, tworzy **Propozycję zmiany** (diff, uzasadnienie, ryzyko, plan cofnięcia) i — za zgodą wg autonomii — wykonuje **naprawę cofalną** z weryfikacją; prowadzi dziennik napraw i raport „Zdrowie systemu”. Naprawia konfigurację, dane i zasoby — nie kod rdzenia; obszaru Jądra nigdy sam (tylko prośba do Brokera).

## Fala i priorytet
F8, P1. Watchdog (F3) restartuje i wchodzi w safe-mode; Diagnosta szuka przyczyny i naprawia.

## Kontrakt (źródło prawdy: `crates/diagnostician-contract`)
```rust
pub enum FailureKind { /* 24: ModuleStartFailure, ConfigCorrupted, SessionDbCorrupted, SessionDbLocked, DiskFull, SidecarCrashLoop, GpuLost, ApiKeyRevoked, RateLimitLoop, BudgetExhausted, PortInUse, ModelMissing, ModelCorrupted, WebViewBroken, SchemaMismatch, UndoJournalFull, FileLocked, DirPermissionDenied, ClockSkew, NetworkDown, UpdatePackageCorrupted*, ResourceBudgetExceeded, LogDiskLimit, KernelConfigTampered* (*Jądro) */ }
pub enum Signal { ModuleState{module, condition, detail}, Error{module, symptom: Symptom, target?, details}, Watchdog(WatchdogSignal), Resource{scope, resource, used, limit, details} }   // Signal::from_event(&Event)
pub fn classify(&[TimedSignal], now, &ClassifierConfig) -> Vec<Detection>      // okno, progi powtórzeń, przyczyna szczegółowa > ogólna
pub fn plan(&Detection, &dyn RepairContext) -> Proposal { diff, rationale, risk, rollback_plan, steps: Vec<RepairStep>, kernel_area, needs_human, verification }
pub enum RepairStep { SetConfig{key, old, new}, RollbackConfig, MoveFile, CopyFile, RemoveCopy, RestartModule, ArchiveEntries, RestoreEntries, QueueDownload, CancelDownload }  // fn inverse()
#[async_trait] pub trait RepairEnv { async fn apply(&RepairStep) -> Result<RepairStep /*pokwitowanie*/>; async fn verify(&Detection) -> Result<bool> }
pub trait RepairContext { /* fakty: config, rewizje, kopie, kwarantanna, porty, fallbacki, archiwum, ścieżki Jądra */ }
#[async_trait] pub trait KernelApprovals { async fn execute(&Proposal) -> KernelOutcome; async fn undo(..) }   // Broker
#[async_trait] pub trait Diagnostician { ingest; scan -> ScanOutcome; approve(id, UserConsent); reject; undo; report() -> HealthReport; repairs; journal }
pub struct DiagnosticianCore<H: DiagHost>   // wspólny dla -impl i -fake; restore(journal) + recover()
```
Zdarzenia: `diagnostician.incident.detected`, `diagnostician.repair.{verified,failed,undone}`, `diagnostician.needs_human`; wejście: `diagnostics.symptom {module, symptom, target, details}`.

## Zależności
`core-bus-contract`, `watchdog-contract` (role, `ConfigHistory`, zegar), `core-config-contract` i `core-registry-contract` (impl).

## Niezmienniki
- Każdy krok ma operację odwrotną; nic nie jest tracone (kwarantanna, archiwum; usuwana tylko kopia identyczna SHA-256 ze źródłem). `SetConfig` = porównaj-i-zamień (cofnięcie nie nadpisuje zmian użytkownika).
- Nieudany krok lub weryfikacja → automatyczne cofnięcie wykonanych kroków; wychładzanie celu; po `max_attempts` — tylko człowiek. Rozwiązane sygnały nie wracają w pętli.
- Obszar Jądra (rodzaje Jądra, moduły `core-*`/Broker/watchdog/updater/compliance, `kernel.*`, ścieżki Jądra): kroki **wyłącznie** przez Brokera (Broker-UI); zgoda użytkownika w panelu nie wystarcza; moduł Jądra nigdy nie jest wyłączany.
- Klucze `diagnostician.*`, `improver.*`, bezpieczeństwa, autonomii, prywatności, budżetów, kosztów, `evals`, kont i sekretów — nigdy (także gdy podsunie je sygnał).
- Dziennik append-only (numeracja bez luk); naprawa przerwana restartem jest cofana przy otwarciu.

## Zdolności / uprawnienia
`fs.write(%LOCALAPPDATA%/Alfa/diagnostyka/**)` (kwarantanna, kopie, archiwum — korzenie `LocalFiles`), konfiguracja przez `core-config` z `Origin::Module("diagnostician")`.

## Izolacja
`inproc`, `always`; skan co 30 s.

## Budżet zasobów
RAM ≤ 6 MB (okno ≤ 10 000 sygnałów); skan < 5 ms.

## Konfiguracja (klucze TOML)
`[diagnostician] autonomy = "auto_low_risk" | "propose_only" | "auto_medium_risk"`, `max_auto_repairs_per_hour = 10`, `retry_cooldown = "15m"`, `max_attempts = 2`, `window = "10m"`, `scan_every = "30s"` — zmienia tylko użytkownik.

## Wkład do UI
Panel „Zdrowie systemu” (`HealthReport`): stan ogólny (ok / ograniczenia / awaria / safe-mode), moduły z zasobami, ostatnie incydenty, co naprawiono („Cofnij”), co wymaga człowieka, karty propozycji (diff, uzasadnienie, ryzyko, plan cofnięcia, „Napraw/Odrzuć”; Jądro → Broker-UI).

## Testy akceptacyjne
- `ACC-F8-diagnostician-01` (F8-01): 24/24 awarie chaosowe (`evals/F8/chaos/catalog.json`) — wykryte z poprawną klasyfikacją, naprawione (sonda), cofnięte do stanu 1:1; Jądro tylko przez Brokera — impl + fake.
- `ACC-F8-diagnostician-02` (F8-06): 100% propozycji z diffem, uzasadnieniem, ryzykiem, planem cofnięcia.
- Kontrakt (impl + fake): autonomia i zgody, porażki i cofanie, Broker, klucze zakazane, konflikty, raport, dziennik; dziennik w pliku i odzysk; property-based (odwrotności, okno klasyfikatora, planista, cofanie ciągów kroków w świecie).

## Fake
`diagnostician-fake`: rdzeń z wirtualnym zegarem + `ChaosWorld` (atrapy portów i Brokera) z 24 awariami i runnerem `run_chaos`.

## Otwarte pytania
- Sondy zdrowia (`HealthProbe`) i restart modułów (`ModuleRestarter`) w `app-*` — przez rejestr i watchdoga.
- Klasteryzacja błędów z `core-log` bez kodów symptomów (heurystyki tekstowe) — SPEC v2; dziś moduły zgłaszają `diagnostics.symptom`.
- „Odtworzenie w piaskownicy” poprawki przed wdrożeniem (PLAN §12.2) — przez bramkę `evals` dla napraw konfiguracji wysokiego ryzyka (v2).
