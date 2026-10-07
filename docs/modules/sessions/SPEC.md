# sessions — SPEC (szkic v0)

## Cel
Sesje (czaty): tworzenie, metadane, dziennik zdarzeń rozmowy (append-only) i projekcja drzewa gałęzi na widoczną rozmowę; wiele okien/kart, fork, przekazanie kontekstu, archiwizacja, eksport (PLAN §11, §6.5, §14.8). Każda sesja ma osobną, szyfrowaną bazę SQLite i osobną pamięć.

## Fala i priorytet
F1. P0. Szablony sesji (Kodowanie, Research, Asystent głosowy, Administracja PC, Pusty) w F1; widok dzielony/odłączane okna — z `ui-shell`.

## Kontrakt (szkic Rust)
```rust
// sessions-contract — SZKIC
pub struct Session { pub id: SessionId /* ULID */, pub title: String, pub model_policy: ModelPolicyRef,
    pub personas: Vec<PersonaId>, pub cast: CastRef, pub memory_scope: MemoryScope, pub cwd: PathBuf,
    pub permission_profile: PermissionProfileRef, pub privacy: PrivacyTag, pub tainted: bool,
    pub autonomy: AutonomyLevel, pub project: Option<ProjectId>, pub pinned: bool, pub archived: bool }
pub struct Turn { pub id: TurnId, pub parent: Option<TurnId>, pub role: Role, pub author: Author,
    pub content: Content, pub heard_prefix: Option<usize>, pub branch: BranchId, pub ts: Timestamp }
pub trait Sessions: Send + Sync {
    fn create(&self, tpl: SessionTemplate) -> Result<Session>;
    fn append(&self, id: SessionId, turn: Turn) -> Result<TurnId>;              // nigdy edit
    fn branch(&self, id: SessionId, from: TurnId) -> Result<BranchId>;           // edytuj/ponów = nowa gałąź
    fn view(&self, id: SessionId, branch: BranchId, range: Range) -> Result<Vec<Turn>>;
    fn fork(&self, id: SessionId, at: TurnId) -> Result<Session>;
    fn delete(&self, id: SessionId, undo_window: Duration) -> Result<UndoToken>;
}
```
Zdarzenia: `session.created`, `session.turn.appended`, `session.branch.created`, `session.cast.changed`, `session.tainted`, `session.archived`, `session.deleted`, `session.context.handoff`.

## Zależności
`core-bus/config/log-contract`, `platform-windows-contract` (ścieżki, Kosz), `search-contract` (indeksowanie), `memory-contract` (zakres), `personas-contract` (F2). Baza: SQLite + SQLCipher (ADR 8, spike i).

## Niezmienniki
- Dziennik tur append-only; „edytuj" i „ponów" tworzą gałęzie; żadna tura nie jest modyfikowana (bloki myślenia dostawców pozostają ważne, PLAN §6.5).
- `assistant_full` i `assistant_heard_prefix` przechowywane osobno; prefiks z flagą `approximate`.
- Zero przecieków między sesjami: osobna baza i klucz per sesja; `forget`/usunięcie sesji kasuje klucz (indeks FTS/wektory giną razem).
- Flaga `tainted` tylko rośnie w obrębie sesji (reset = nowa sesja/gałąź jawnie).
- Usunięcie sesji cofalne przez 10 s (Kosz logiczny), potem crypto-shredding.
- Przekazanie kontekstu między czatami to jawna tura `Author::Handoff`, nie współdzielenie pamięci.

## Zdolności / uprawnienia
`fs.read/write(%LOCALAPPDATA%\Alfa\sessions\**)` i `%USERPROFILE%\Alfa\Sesje\<nazwa>\**` (katalog roboczy) — przez jądro.

## Izolacja
`inproc`, `always`.

## Budżet zasobów
RAM ≤ 10 MB przy 20 otwartych sesjach (tury ładowane stronami); `view` 1000 tur ≤ 50 ms; przyrost Private WS ≤ 5% po 500 wiadomościach (ACC F1).

## Konfiguracja (klucze TOML)
`[sessions] default_template = "pusty"`, `workdir_root = "%USERPROFILE%\\Alfa\\Sesje"`, `auto_title = true`, `delete_undo_seconds = 10`, `default_privacy = "normal"`.

## Wkład do UI
Panel Sesje (lista, wyszukiwanie, projekty, tagi, przypięte, kropka aktywności, nieprzeczytane); pasek górny (ścieżka projekt ▸ sesja, stan); warianty `‹ 1/3 ›` i gałęzie w rozmowie.

## Testy akceptacyjne
- `ACC-F1-sessions-01`: ≥ 3 sesje równolegle, 0 przecieków (testy szpiegowskie: treść sesji A nieobecna w bazie/indeksie sesji B).
- `ACC-F1-sessions-02`: property-based — dowolna sekwencja append/branch/fork daje spójne drzewo, brak modyfikacji istniejących tur.
- `ACC-F1-sessions-03`: przełączenie sesji z 1000 wiadomości ≤ 150 ms (z `ui-shell`).
- `ACC-F1-sessions-04`: round-trip `.alfa` sesji (z `transfer`).

## Fake
`sessions-fake`: sesje w pamięci, fixture'y rozmów (w tym gałęzie i prefiksy barge-in), deterministyczne ULID.

## Otwarte pytania
- Schemat IR tury (wersjonowany; wspólny z `providers-api` do renderowania historii per dostawca) — do ustalenia w SPEC v1 i ADR (6).
- Kompaktowanie okna kontekstu: w `sessions` czy `memory` (warstwa robocza) — do ustalenia w SPEC v1.

## Zmiany po implementacji (F1, `sessions-contract/-impl/-fake`, 2026-09-30)
- **Kontrakt synchroniczny** (SQLite blokuje; z async — `spawn_blocking`), podzielony na `SessionCatalog`
  (katalog) i `SessionHistory` (historia) + `Sessions` = oba. W kontrakcie także `KeyVault` (sejf kluczy;
  produkcyjny Credential Manager — `platform-windows`) i `SessionDbProvider` (dostęp `search`/`memory`/
  `artifacts` do tej samej bazy sesji, `Arc<lib_sqlstore::Db>`).
- Operacje historii: `append_turn(id, parent, NewTurn)` — **tylko do liścia** (`NotALeaf`), pierwsza tura
  z `parent = None` (`RootExists` dla kolejnych); `fork_from(id, sibling_of, NewTurn)` — „edytuj/ponów”
  = wariant (ten sam rodzic) w **nowej gałęzi**; `branch_projection(id, leaf)` zamiast `view(branch, range)`
  (stronicowanie — później); `siblings` (`‹ 1/3 ›`), `latest_leaf`, `set_active_leaf`/`active_leaf`,
  `set_hidden` (flaga widoku w osobnej tabeli), `save_draft`/`draft`, `turn_count`.
- **Usłyszany prefiks** = osobny fakt append-only (`turn_heard`), `HeardPrefix { chars, approximate }`
  liczony w znakach `assistant_full` (= `content.text`); można go podać przy zapisie albo dopisać **raz**
  później (TTS kończy odtwarzanie po zapisie tury). Treść tury nigdy się nie zmienia.
- `fork(id, at)` do nowej sesji i `session.context.handoff` — poza v0 (UI/`transfer`). `delete(id,
  undo_window)` → `trash_session`/`restore_session` (okno 10 s liczy wywołujący) + `delete_session`
  (crypto-shredding: klucz z sejfu → pliki bazy z `-wal`/`-shm` → wpis katalogu; sieroty sprzątane przy starcie).
- Metadane v0: bez `cast`, `memory_scope`, `permission_profile`, `autonomy` (należą do `personas`/Brokera —
  dojdą z tymi modułami); dodane `template`, `trashed`, `tags`, `created_at/updated_at`. `tainted` tylko rośnie
  (`mark_tainted`; brak pola w `SessionPatch`).
- Identyfikatory: sesja UUIDv7 (atrapa `sess-NNNN`); tura i gałąź — kolejne liczby od 1 w obrębie sesji.
- Magazyn: `index.db` (katalog + liczniki listy) i `<id>.db` per sesja; `turns.body` = kanoniczny JSON
  niezmiennej części tury; **wyzwalacze odrzucają `UPDATE`/`DELETE`** na `turns`, `branches`, `turn_heard`.
  Liczniki w `index.db` aktualizowane po transakcji tury (dwie bazy — bez atomowości; przeliczanie — później).
- Indeksowanie tur przez `search_contract::TxIndexer` w tej samej transakcji co zapis tury.
- Zdarzenia dodatkowe: `session.updated`, `session.trashed`, `session.restored`; ładunki bez treści tur.
- Pomiary (debug, kontener 4 rdzenie współdzielony z innymi buildami): 1000 tur `append_turn` 0,36–0,39 s,
  projekcja 1000 tur 9–10 ms, lista sesji 0,14 ms. Z indeksowaniem FTS+wektor (`search-impl`) dochodzi
  0,5–0,9 s/1000 tur w debug (kod C bez optymalizacji) — zalecane `[profile.dev.package.libsqlite3-sys]
  opt-level = 3` w root `Cargo.toml`.

## Zmiany dla `transfer` (F1, 2026-10-01; addytywne)
- `SessionHistory::all_turns(id)` — całe drzewo rosnąco po `id` (domyślnie przez `turn_count` + `turn`; `-impl` jednym
  zapytaniem).
- `SessionCatalog::adopt_session(PortableSession { meta, turns, active_leaf, draft })` — import z zachowaniem `id`,
  metadanych i znaczników czasu, **atomowo** (baza sesji w jednej transakcji, wpis katalogu na końcu; po awarii plik bez
  wpisu sprząta `sweep_orphans`); nowy klucz w sejfie tej maszyny; `id` musi spełniać `is_portable_session_id`
  (`[A-Za-z0-9_-]{1,64}` — trafia do nazwy pliku); istniejąca sesja → nowy błąd `SessionError::AlreadyExists`.
- `SessionHistory::import_turns(id, &[Turn])` — dopisanie partii pełnych tur (scalanie przy imporcie) z regułami
  `TreeCursor` (kolejne `id` bez luk; gałąź jak przy `append_turn`/`fork_from`), wszystko albo nic, indeksowanie w tej
  samej transakcji, zdarzenie `session.turns.imported`; tury importowane nie zwiększają licznika nieprzeczytanych.
- Domyślne implementacje `adopt_session`/`import_turns` zwracają `Invalid` („nieobsługiwane”) — istniejące
  implementacje traitów się kompilują. Testy kontraktowe: +3 przypadki (round-trip, odrzucenia, atomowość).

## Fala 5 — powrót do starszej wersji po migracji (m-23, ADR 0007)
- **Problem:** po aktualizacji z migracją bazy (np. `search` 0002) powrót do starszej wersji (aktualizator, wersje obok
  siebie) dawał `UnknownMigration` — sesja nie otwierała się wcale, choć ADR 0007 wymaga, by rollback nie niszczył danych.
- **Zachowanie (`lib_sqlstore::migrate`):** migracje zapisane w bazie, nieznane kodowi i **nowsze od każdej znanej** =
  baza z nowszej wersji. `migrate` nic nie zapisuje (schemat bez zmian) i zwraca `Ok` z `MigrationReport { newer,
  read_only }`. Gdy nowsza wersja oznaczyła wszystkie te migracje jako **addytywne** (`migrate_with(.., additive)` →
  tabela `schema_compat(namespace, version)`), starsza pracuje normalnie; inaczej połączenie przechodzi w `PRAGMA
  query_only` — historia czytelna, każdy zapis odrzucany przez SQLite. Luka/rozwidlenie historii (nieznana wersja nie
  nowsza od znanych) nadal `UnknownMigration`; znana oczekująca + nieznana nowsza → `MigrationOutOfOrder`; migracja
  innej przestrzeni nazw na połączeniu tylko do odczytu → `StoreError::ReadOnly` (nic nie zapisano).
- **`sessions-impl`:** zapis do bazy tylko do odczytu → `Storage` z komunikatem „baza sesji pochodzi z nowszej wersji
  Alfy i jest tylko do odczytu — zaktualizuj Alfę…” (`rows::db_err`); `open_db` toleruje błąd `TxIndexer::prepare`
  na połączeniu tylko do odczytu (np. `search` w trakcie przebudowy wektorów chce pisać) — historia ma się otworzyć,
  wyszukiwanie w tej sesji wraca po aktualizacji.
- **Zasada dla nowych migracji (dla autorek modułów):** migracja jest addytywna tylko, gdy dokłada tabele, indeksy albo
  kolumny z wartością domyślną — bez nowych wyzwalaczy, ograniczeń na starych tabelach i niezmienników łączących stare
  tabele z nowymi (starsza wersja dopisuje wiersze bez wiedzy o nowych tabelach, a nowsza po ponownej aktualizacji musi
  je przyjąć). Wszystko inne → bez oznaczenia (starsza wersja: tylko odczyt). Każda nowa migracja dokłada fixture
  i scenariusz w `evals/F7/migrations` (zasada utrzymania F7-08).
- **Dlaczego nie kopia bazy przed migracją:** przywrócenie kopii po rollbacku gubi tury dopisane w nowszej wersji
  (sprzeczne z append-only i ADR 0007), a kopie per sesja mnożą zaszyfrowane pliki do sprzątania. Tryb tylko do odczytu
  nie traci niczego; dane wracają do pełnej pracy po ponownej aktualizacji.
- **Propozycja dla Jądra (`updater`, decyzja człowieka — niewdrożona):** przed `rollback_by` aktualizator ostrzega
  „sesje zmigrowane przez nowszą wersję będą tylko do odczytu, dopóki nie wrócisz do nowszej” (lista przestrzeni
  nazw z nieaddytywnymi migracjami — moduł mógłby ją publikować w `module.toml` albo w pliku obok bazy); UI aplikacji
  pokazuje baner przy sesji tylko do odczytu (`MigrationReport::read_only` → zdarzenie/stan sesji — osobne zadanie dla
  `app-*`). Bez tej zmiany zachowanie jest już bezpieczne (fail-closed dla zapisu, odczyt działa).
- Testy: `lib-sqlstore/tests/rollback.rs` (tylko odczyt bez zmian schematu, addytywna → zapis, mieszane → tylko
  odczyt, luka, `ReadOnly` innej przestrzeni), `sessions-impl/tests/rollback.rs` (historia czytelna po rollbacku,
  czytelny błąd zapisu, indeks bez `prepare`, addytywna → dopisywanie), `sessions-impl/tests/f7_migrations.rs` (runner
  scenariuszy `sqlite.sessions`: m-20, m-23).
