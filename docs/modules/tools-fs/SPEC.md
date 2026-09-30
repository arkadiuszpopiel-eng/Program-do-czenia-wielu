# tools-fs — SPEC (szkic v0)

## Cel
Narzędzia plikowe agentek: odczyt/zapis/kopiowanie/przenoszenie/usuwanie (do Kosza), katalogi, atrybuty, wyszukiwanie, hashe, archiwa, obserwatory zmian — każde wywołanie z tokenem `fs.read/write(zakres)` od Brokera, zapisane w dzienniku cofania, ze wskazaniem odwracalności i faktów dla klasyfikatora ryzyka (PLAN §7.2, §8.1, §8.7).

## Fala i priorytet
F3 (podstawowe: list/read/write/move/copy/delete/mkdir/search/hash). P0. Archiwa, ACL, dowiązania, udziały, OneDrive — F6 (`tools-fs` v1).

## Kontrakt (szkic Rust)
```rust
// tools-fs-contract — SZKIC (narzędzia = ToolSpec dla LLM + implementacja)
pub enum FsTool { List { path, depth }, Read { path, range }, Write { path, content, mode: Create | Overwrite | Append }, Move { from, to }, Copy { from, to },
                  Delete { path, recursive } /* do Kosza */, Mkdir { path }, Search { root, pattern, content: Option<String> }, Hash { path, algo }, Stat { path } }
pub struct FsToolSpec { pub name: &'static str, pub schema: JsonSchema, pub reversible: Reversibility, pub required_cap: fn(&FsTool) -> Capability }
pub struct FsCallCtx { pub token: CapToken, pub session: SessionId, pub run: RunId, pub step: StepId, pub undo_tx: TxId }
pub trait ToolsFs: Send + Sync {
    fn specs(&self) -> Vec<FsToolSpec>;
    fn facts(&self, call: &FsTool) -> ActionFacts;                          // dla risk-classifier (odwracalność, zakres, bulk)
    fn call(&self, call: FsTool, ctx: FsCallCtx) -> Result<ToolResult, ToolError /* Denied | OutOfScope | DenyList | Io */>;
}
```
Zdarzenia (strumień Narzędzia i GUI; Audyt dla usuwania/poza profilem): `tool.fs.called`, `tool.fs.result`, `tool.fs.denied`, `tool.fs.denylist_hit`.

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
Kroki narzędzi w wiadomości (jedna linia + rozwinięcie), karty „Cofnij", podgląd plików przez `artifacts`, Ustawienia → Pliki.

## Testy akceptacyjne
- `ACC-F3-tools-fs-01`: ≥ 200 losowych operacji (property-based) → 100% cofalne przez `undo-journal`.
- `ACC-F3-tools-fs-02`: ≥ 100 prób poza zakresem tokenu / na deny-liście (w tym dowiązania i `..`) = 0 wykonanych.
- `ACC-F3-tools-fs-03`: eval narzędzi fs na lokalnym modelu 3–4,5B ≥ próg ustalony w F0 (zestaw zamrożony).

## Fake
`tools-fs-fake`: na wirtualnym FS z `platform-windows-fake`, tokeny z `safety-broker-fake`, deterministyczne wyniki.

## Otwarte pytania
- Wyszukiwanie: Windows Search (indeks) vs własne przeszukiwanie — do ustalenia w SPEC v1.
- Format `ToolSpec` wspólny dla wszystkich `tools-*` i MCP (JSON Schema, `strict`) — do ustalenia w SPEC v1 (`tools-common-contract`?).
