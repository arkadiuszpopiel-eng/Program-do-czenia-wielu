# undo-journal — SPEC (v1: kontrakt i rdzeń zaimplementowane)

## Cel
Dziennik cofania dla operacji `fs.*` (pre-image, Kosz, snapshot zakresu dla shella), „Cofnij” jednym kliknięciem (toast 8 s, karty kroków); flaga `reversible: yes|scoped|no` z manifestu narzędzia jako wejście klasyfikatora (PLAN §8.7, §14.8, §8.3). VSS dla operacji masowych — poza v1 części 1.

## Fala i priorytet
F3. P0. Mosty CLI: snapshot przed/po (F4) przez ten sam kontrakt.

## Kontrakt (źródło prawdy: `crates/undo-journal-contract`)
```rust
pub trait UndoJournal: Send + Sync {
    fn begin_step(&self, ctx: StepCtx /* session, agent, run, turn, label, allow_irreversible */) -> Result<StepId, UndoError>;
    fn write / copy / move_path / delete /* Kosz */ / delete_permanent(&self, step, ...) -> Result<(), UndoError>;
    fn snapshot_scope(&self, step, root: &Path) -> Result<(), UndoError>;   // przed poleceniem powłoki
    fn commit_step(&self, step) -> Result<StepSummary /* „Delta: przeniesiono 14 plików” */, UndoError>;
    fn abort_step(&self, step) -> Result<UndoReport, UndoError>;          // cofa niezatwierdzony krok
    fn undo(&self, step) -> Result<UndoReport, UndoError>;
    fn undo_last(&self, session, n) -> Result<Vec<UndoReport>, UndoError>;
    fn steps(&self, session) -> Vec<StepSummary>;  fn prune(&self) -> usize;
}
pub enum UndoOp { Write{path, before, pre_image, after}, Copy{from, to, after}, Move{from, to, after}, Delete{path, before, pre_image},
                  DeletePermanent{path, before, pre_image}, ScopeSnapshot{root, files: Manifest} }
pub struct JournalEntry { step, seq, op, platform_undo: Option<UndoToken>, boot }
pub enum UndoError { UnknownStep, BadState, Expired, Conflict{path, expected, found}, PreImageTooLarge, StoreFull, SnapshotTooLarge, Partial(UndoReport), Platform, Store }
```
Rdzeń `Journal` (w kontrakcie, jak `scheduler-lite`) nad `FsPort` i `JournalStore` (`MemStore` / `DirStore` w `-impl`).
Zdarzenia: `undo.recorded`, `undo.undone {ok, partial}`, `undo.snapshot.created`, `undo.pruned`, `undo.failed`.

## Zależności
`core-bus-contract`, `platform-contract` (`FsPort`, `UndoToken`); `sha2 0.10.9`. Wywołujący: `tools-fs`, `tools-shell`, UI.

## Niezmienniki
- Kolejność: pre-image → operacja platformy → wpis; błąd wpisu cofa operację tokenem platformy (brak wpisu = brak operacji); nieudana operacja nie zmienia stanu.
- Cofnięcie: najpierw kontrola konfliktów całego kroku (stan bieżący = stan „po” ostatniej operacji kroku; dla snapshotu — manifest „po” z zatwierdzenia); konflikt → czytelny błąd, nic nie jest ruszane. Potem odwrotna kolejność: token platformy z tego uruchomienia, inaczej pre-image (także po restarcie).
- Częściowe niepowodzenie → `UndoError::Partial` z listą tego, co nie wróciło; pre-image zostają.
- Trwałe usunięcie i nadpisanie bez pre-image (za duże / pełny magazyn) wymagają `allow_irreversible` (po zgodzie Brokera); usunięcie do Kosza ma pre-image jako zapas.
- Snapshot zakresu = kopia plików z limitem (decyzja: prostsze i bezpieczniejsze niż shadow-git — bez zewnętrznego narzędzia, przywracanie przez `FsPort`); za duży zakres → `SnapshotTooLarge` (shell poza snapshotem = `reversible: no` → potwierdzenie).
- Retencja i limit magazynu wypierają najstarsze kroki (stają się `Expired`); pre-image współdzielone (dedup SHA-256) usuwane dopiero, gdy nieużywane.
- Dziennik obowiązuje także na L4.

## Izolacja / budżet
`inproc`, `lazy`. Narzut ≤ 5 ms + kopia pre-image; limit 50 MB/plik, magazyn 2 GB, retencja 7 dni.

## Konfiguracja (klucze TOML)
`[undo] pre_image_max_mb = 50`, `store_limit_gb = 2`, `retention = "7d"`, `snapshot_max_files = 10000`, `snapshot_max_mb = 500`, `toast_seconds = 8`, `shell.snapshot_scope = true` (kernel_policy).

## Testy akceptacyjne
- `ACC-F3-undo-journal-01`: 3 × 256 losowych sekwencji `fs.*` (tokeny platformy / wyłącznie pre-image / restart) + 200 na dysku z restartem → 100% przywrócenia.
- `ACC-F3-undo-journal-02`: snapshot zakresu → zmiany „skryptu” (nowe, zmienione, usunięte pliki) cofnięte (test kontraktowy); zestaw ≥ 50 skryptów (F3-03) — `evals/`.
- `ACC-F3-undo-journal-03`: chaos — awaria w trakcie cofania → raport częściowy, pre-image nietknięte (`undo-journal-fake`).

## Fake
`undo-journal-fake`: rdzeń w pamięci nad dowolnym `FsPort`, `FlakyFs` (awarie operacji i przywracania), awaria zapisu dziennika.

## Otwarte pytania
- Szyfrowanie pre-image kluczem sesji (crypto-shredding) — integracja z `sessions` `KeyVault` (następny krok).
- VSS dla operacji masowych (uprawnienia admina) — przez Broker/UAC lub rezygnacja w v1.
