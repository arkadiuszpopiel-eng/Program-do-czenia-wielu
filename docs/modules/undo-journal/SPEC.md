# undo-journal — SPEC (szkic v0)

## Cel
Dziennik cofania dla operacji `fs.*` (pre-image, przeniesienia do Kosza, shadow-git/snapshot zakresu dla shella), „Cofnij" jednym kliknięciem (toast 8 s, karty kroków), VSS tylko dla operacji masowych; flaga `reversible: yes|scoped|no` z manifestu narzędzia jako wejście klasyfikatora ryzyka (PLAN §8.7, §14.8, §8.3).

## Fala i priorytet
F3. P0. Mosty CLI: snapshot przed/po (F4) przez ten sam kontrakt.

## Kontrakt (szkic Rust)
```rust
// undo-journal-contract — SZKIC
pub enum Reversibility { Yes, Scoped(Scope), No }
pub enum UndoOp { FileWrite { path, pre_image: BlobRef }, FileMove { from, to }, FileDelete { path, recycle_id }, DirCreate { path },
                  ShellScoped { scope: Scope, snapshot: SnapshotId }, Bulk { ops: Vec<UndoOp>, vss: Option<VssId> } }
pub struct UndoEntry { pub id: UndoId, pub session: SessionId, pub run: RunId, pub step: StepId, pub persona: PersonaId, pub op: UndoOp, pub ts: Timestamp, pub undone: bool }
pub trait UndoJournal: Send + Sync {
    fn begin(&self, ctx: StepCtx) -> Result<TxId>;
    fn record(&self, tx: TxId, op: UndoOp) -> Result<()>;
    fn commit(&self, tx: TxId) -> Result<UndoId>;
    fn undo(&self, id: UndoId) -> Result<UndoReport>;                    // odwrotna kolejność, atomowo gdzie możliwe
    fn snapshot_scope(&self, scope: &Scope) -> Result<SnapshotId>;       // shadow-git / kopia
    fn list(&self, session: SessionId) -> Vec<UndoEntry>;
}
```
Zdarzenia (Audyt): `undo.recorded`, `undo.undone { ok, partial }`, `undo.snapshot.created`, `undo.pruned`, `undo.failed`.

## Zależności
`core-bus/config/log-contract`, `platform-windows-contract` (Kosz, VSS, pliki), `tools-fs-contract`/`tools-shell-contract` (wywołujący), `notify-contract` (toast „Cofnij"), `safety-broker-contract` (Audyt).

## Niezmienniki
- Każda operacja `fs.*` agentki przechodzi przez transakcję dziennika; brak wpisu = narzędzie nie wykonuje operacji (kontrakt `tools-fs`).
- Usuwanie domyślnie do Kosza; pre-image plików do limitu rozmiaru (większe → VSS lub `Scoped` z ostrzeżeniem).
- `undo` odtwarza stan w odwrotnej kolejności; przy częściowym niepowodzeniu raport wskazuje, co nie wróciło (nigdy cicho).
- Shell w zakresie: snapshot zakresu przed wykonaniem; poza zakresem — potwierdzenie w Broker-UI.
- Dziennik cofania obowiązuje także na L4 (PLAN §8.3).
- Pre-image szyfrowane kluczem sesji (crypto-shredding jak logi); retencja i limit dysku.

## Zdolności / uprawnienia
Działa w zakresie tokenu operacji pierwotnej; `undo` wykonywane jako Ty z UI (bez tokenu agentki).

## Izolacja
`inproc`, `lazy`.

## Budżet zasobów
Narzut na operację fs ≤ 5 ms + kopia pre-image (limit domyślnie 50 MB/plik); magazyn ≤ 2 GB z rotacją; VSS tylko masowo (≥ N plików).

## Konfiguracja (klucze TOML)
`[undo] pre_image_max_mb = 50`, `store_limit_gb = 2`, `retention = "7d"`, `vss_threshold_files = 200`, `toast_seconds = 8`, `shell.snapshot_scope = true` (kernel_policy).

## Wkład do UI
Toast „Cofnij" po cofalnej akcji, przycisk „Cofnij" na kartach kroków („Delta: przeniesiono 14 plików · 2,1 s · Cofnij"), lista w Osi czasu, Ustawienia → Pliki.

## Testy akceptacyjne
- `ACC-F3-undo-journal-01`: ≥ 200 losowych operacji `fs.*` (property-based: zapis/przeniesienie/usunięcie/katalogi) → undo przywraca stan 100%.
- `ACC-F3-undo-journal-02`: snapshot zakresu dla shella → po skrypcie modyfikującym pliki undo przywraca zakres.
- `ACC-F3-undo-journal-03`: chaos — przerwanie w trakcie `undo` → raport częściowy, brak utraty pre-image.

## Fake
`undo-journal-fake`: dziennik w pamięci na wirtualnym FS (`platform-windows-fake`), sterowane błędy przywracania.

## Otwarte pytania
- Shadow-git vs kopia katalogu dla snapshotu zakresu (duże drzewa) — do ustalenia w SPEC v1.
- VSS wymaga uprawnień admina? — sprawdzić; jeśli tak, przez Broker/UAC lub rezygnacja z VSS w v1.
