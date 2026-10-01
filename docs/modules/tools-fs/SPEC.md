# tools-fs — SPEC (v0, zaimplementowany)

## Cel
Narzędzia plikowe agentek: odczyt/zapis/kopiowanie/przenoszenie/usuwanie (do Kosza), katalogi, atrybuty, wyszukiwanie, hashe, archiwa, obserwatory zmian — każde wywołanie z tokenem `fs.read/write(zakres)` od Brokera, zapisane w dzienniku cofania, ze wskazaniem odwracalności i faktów dla klasyfikatora ryzyka (PLAN §7.2, §8.1, §8.7).

## Fala i priorytet
F3 (podstawowe: list/read/write/move/copy/delete/mkdir/search/hash). P0. Archiwa, ACL, dowiązania, udziały, OneDrive — F6 (`tools-fs` v1).

## Kontrakt
Format wspólny: `tools-common` (docs/modules/tools-common/SPEC.md) — manifest, `Tool::call(args, ToolCtx) → ToolOutcome`, `BrokerGate`.
```rust
// tools-fs-contract: 11 narzędzi (FsToolKind), argumenty zamknięte (deny_unknown_fields)
fs_list { path, depth ≤ 3 }      fs_read { path, offset?, max_bytes? }   fs_stat { path }   fs_search { root, pattern /* glob * ? */, content? }
fs_write { path, content, mode: create | overwrite | append }   fs_move / fs_copy { from, to /* cel nie istnieje */ }   fs_rename { path, new_name }
fs_delete { path } /* do Kosza, cofalne */   fs_delete_permanent { path } /* NeedsConfirmation + intencja fs.confirm_delete_permanent */   fs_mkdir { path }
pub fn manifest(kind) -> ToolManifest;  pub fn manifests() -> Vec<ToolManifest>;  pub struct FsToolsConfig { read_max_bytes, write_max_bytes, list_max_entries, search_* , output_max_chars }
// tools-fs-impl
pub struct FsTools; impl FsTools { pub fn new(deps: FsToolsDeps /* broker, journal, fs: FsPort, env: PathEnv, deny: DenyLists, config, bus */) -> Self } impl Toolset for FsTools
```
Zdolności: `fs.read` (list/read/stat/search), `fs.write` (mutacje), `fs.copy` = oba; grupy ról `fs` / `fs.read` / `fs.write`. Wynik `fs_list/read/search` — niezaufany (`TaintSource::File`). Mutacja = krok dziennika cofania (`UndoRef { service: Journal }`, tekst „<agentka>: <tytuł>”).

## Zależności
`core-bus/config/log-contract`, `platform-windows-contract` (`FsPort`), `safety-broker-contract` (verify tokenu), `undo-journal-contract`, `risk-classifier-contract` (fakty), `artifacts-contract` (rejestracja plików w katalogu sesji).

## Niezmienniki
- Brak wywołania bez zweryfikowanego tokenu o zakresie pokrywającym ścieżkę (kanonizowaną, po rozwiązaniu dowiązań); `OutOfScope` bez próby wykonania.
- Deny-lista ścieżek poświadczeń (`~/.claude`, `~/.codex`, profile przeglądarek, Credential Manager) — twarda blokada niezależnie od tokenu i poziomu.
- Usuwanie domyślnie do Kosza; każda mutacja w transakcji `undo-journal` (brak wpisu = brak operacji); zapis atomowy.
- Treść odczytanych plików jest niezaufanym wejściem: wynik oznaczony `untrusted`, sesja może stać się `tainted` (decyzja w `agent-runtime`/Brokerze).
- Duże pliki: odczyt zakresami, limit rozmiaru w wyniku dla LLM; wynik w całości do artefaktu.
- Narzędzie wykonuje się z restricted token / AppContainer dla ≤ L3 (przez `ProcessPort`, gdy operacja delegowana do procesu).

## Zdolności / uprawnienia
`fs.read(zakres)`, `fs.write(zakres)` — zakres z tokenu (np. `%USERPROFILE%\Downloads\**`); `Search` w indeksie Windows = `fs.read(root)`.

## Izolacja
`inproc` (wywołania przez `SystemPort`), `lazy`.

## Budżet zasobów
Narzut na wywołanie ≤ 5 ms + dziennik cofania; RAM ≤ 3 MB; listowanie 10k plików ≤ 200 ms.

## Konfiguracja (klucze TOML)
`[tools.fs] recycle_bin = true`, `read_max_kb = 512`, `list_max_entries = 5000`, `[tools.fs.denylist]` (kernel_policy, współdzielona z `[platform.denylist_paths]`).

## Wkład do UI
Kroki narzędzi w wątku (jedna linia + „Cofnij”), Replay krok po kroku, toast „Cofnij” (8 s), karta intencji „trwałe usunięcie” (właściciel potwierdza poza pętlą), Ustawienia → Pliki.

## Integracja w aplikacji
`app-agents::AgentTools` składa `FsTools` nad Brokerem w procesie (przez `TicketLog` — fakty karty zatwierdzenia), dziennikiem cofania `undo-journal-impl` i `FsPort` z `platform-windows` (ten sam port co cofanie). Zakres = katalog roboczy sesji wybrany przez właściciela (`sessions_choose_workdir`; katalogi danych Alfy, deny-lista i segmenty poświadczeń odrzucane). „Cofnij” w UI → `turns_undo_step` → `Broker::undo_step` → dziennik.

## Testy akceptacyjne
- `ACC-F3-tools-fs-01`: ≥ 200 losowych operacji (property-based) → 100% cofalne przez `undo-journal`.
- `ACC-F3-tools-fs-02`: ≥ 100 prób poza zakresem tokenu / na deny-liście (w tym dowiązania i `..`) = 0 wykonanych.
- `ACC-F3-tools-fs-03`: eval narzędzi fs na lokalnym modelu 3–4,5B ≥ próg ustalony w F0 (`evals/F3/tools/`, zadania `fs-*`; CI: format + zadania skryptowane).

## Fake
`tools-fs-fake`: na wirtualnym FS z `platform-fake`, decyzje z `safety-broker-fake`, deterministyczne wyniki (`ScriptedTool` z `tools-common`).

## Otwarte pytania
- Wyszukiwanie: Windows Search (indeks) vs własne przeszukiwanie (v0: własne, z limitami) — SPEC v1.
- Hashe, archiwa, obserwatory zmian — F6 (`tools-fs` v1).
