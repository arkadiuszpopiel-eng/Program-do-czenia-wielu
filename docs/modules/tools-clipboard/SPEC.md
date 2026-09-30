# tools-clipboard — SPEC (szkic v0)

## Cel
Schowek dla agentek i UI: odczyt/zapis wielu formatów (tekst, HTML, obraz, pliki CF_HDROP), **historia schowka** z retencją i wykluczeniami (hasła, aplikacje z deny-listy), wklejanie do composera z palety poleceń; narzędzie MCP dla mostów CLI od F4 (PLAN §7.2, §14.8, §8.5, §16.2).

## Fala i priorytet
F3 (narzędzie agentki + historia w palecie). F4: eksponowane przez serwer MCP v0. P0.

## Kontrakt (szkic Rust)
```rust
// tools-clipboard-contract — SZKIC
pub enum ClipFormat { Text, Html, Image(PngRef), Files(Vec<PathBuf>) }
pub struct ClipItem { pub id: ClipId, pub formats: Vec<ClipFormat>, pub ts: Timestamp, pub source_app: Option<String>, pub pinned: bool, pub excluded_reason: Option<ExcludeReason> }
pub enum ClipTool { Get, Set { content: ClipFormat }, History { limit }, Pin { id }, Clear }
pub trait ToolsClipboard: Send + Sync {
    fn spec(&self) -> ToolSpec;                                  // reversible: Yes (poprzednia zawartość w dzienniku)
    fn call(&self, call: ClipTool, ctx: ToolCallCtx) -> Result<ToolResult, ToolError>;
    fn history(&self, caller: Caller /* Owner | Agent */) -> Vec<ClipItem>;   // Agent: tylko bieżąca zawartość, bez historii (do ustalenia)
    fn watch(&self) -> Subscription<ClipItem>;
}
```
Zdarzenia: `tool.clipboard.get/set` (Narzędzia i GUI; treść redagowana), `clipboard.history.added`, `clipboard.history.excluded` (powód: hasło/deny-lista), `clipboard.history.pruned`.

## Zależności
`core-bus/config/log-contract`, `platform-windows-contract` (`ClipboardPort`, `watch`, `IsPassword` z UIA okna źródłowego — od F6, wcześniej deny-lista aplikacji), `safety-broker-contract`, `undo-journal-contract` (poprzednia zawartość przy `Set`), `search-contract` (opcjonalne wyszukiwanie w historii — tylko Owner).

## Niezmienniki
- Wpisy z pól haseł (UIA `IsPassword`, gdy dostępne) i z aplikacji/okien z deny-listy nigdy nie trafiają do historii ani do agentek.
- Zawartość schowka to niezaufane wejście dla agentki (taint), jeśli pochodzi spoza Alfy.
- `Set` przez agentkę zapisuje poprzednią zawartość do dziennika cofania; toast „Cofnij".
- Historia szyfrowana w spoczynku, retencja domyślnie 7 dni, limit wpisów; `Clear` natychmiastowe.
- Agentka bez tokenu `clipboard` (zdolność do ustalenia: osobna czy `fs.read`-podobna) nie odczytuje schowka; historia dostępna tylko dla właściciela (UI).
- Brak przesyłania obrazów przez IPC — przez ścieżki/protokół zasobów.

## Zdolności / uprawnienia
`clipboard.read` / `clipboard.write` (propozycja nowej zdolności — do potwierdzenia w SPEC v1 `safety-broker`; PLAN nie wymienia wprost).

## Izolacja
`inproc`, `lazy` (obserwator schowka aktywny, gdy historia włączona).

## Budżet zasobów
RAM ≤ 3 MB + cache obrazów (≤ 100 MB na dysku); reakcja na zmianę schowka ≤ 50 ms; brak pollingu.

## Konfiguracja (klucze TOML)
`[tools.clipboard] history = true`, `retention = "7d"`, `max_items = 500`, `store_images = true`, `image_max_mb = 10`, `[tools.clipboard.denylist] apps = ["KeePass.exe", ...]`, `windows = [...]`.

## Wkład do UI
Sekcja „Historia schowka" w palecie `Ctrl+K` (wklej do composera), „Kopiuj jako plik" w artefaktach, `Ctrl+Shift+V` wklej jako tekst, Ustawienia → Komputer → Schowek.

## Testy akceptacyjne
- `ACC-F3-tools-clipboard-01`: wpis z aplikacji na deny-liście / pola hasła (fixture UIA) → 0 wpisów w historii (100/100).
- `ACC-F3-tools-clipboard-02`: `Set` przez agentkę → „Cofnij" przywraca poprzednią zawartość (formaty: tekst, HTML, obraz, pliki).
- `ACC-F4-tools-clipboard-03`: narzędzie przez serwer MCP v0 działa dla mostu CLI z tokenem sesyjnym; bez tokenu = odmowa.

## Fake
`tools-clipboard-fake`: schowek i historia w pamięci (`platform-windows-fake`), skryptowane zmiany z aplikacji źródłowych.

## Otwarte pytania
- Czy agentka widzi historię, czy tylko bieżącą zawartość — do ustalenia w SPEC v1 (propozycja: tylko bieżąca).
- Nazwa zdolności schowka w katalogu zdolności Brokera.
