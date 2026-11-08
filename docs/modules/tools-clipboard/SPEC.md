# tools-clipboard — SPEC (v0 narzędzia agentki zaimplementowane; historia — szkic)

## Cel
Schowek dla agentek i UI: odczyt/zapis wielu formatów (tekst, HTML, obraz, pliki CF_HDROP), **historia schowka** z retencją i wykluczeniami (hasła, aplikacje z deny-listy), wklejanie do composera z palety poleceń; narzędzie MCP dla mostów CLI od F4 (PLAN §7.2, §14.8, §8.5, §16.2).

## Fala i priorytet
F3 (narzędzie agentki + historia w palecie). F4: eksponowane przez serwer MCP v0. P0.

## Kontrakt
Narzędzia agentki (v0, format `tools-common`, docs/modules/tools-common/SPEC.md):
```rust
// tools-clipboard-contract
clipboard_read {}                                   // bieżąca zawartość: tekst | obraz PNG (rozmiar) | lista plików — wynik niezaufany (taint), bez historii
clipboard_write { text? | image_png_base64? }       // poprzednia zawartość → rejestr cofania (UndoRef { service: Clipboard })
pub trait ClipboardUndo { fn undo(&self, id: u64) -> Result<(), ClipboardUndoError /* Unknown | Conflict | Platform */>; }
pub struct ClipboardToolsConfig { read_max_chars, image_max_bytes, write_max_chars, undo_depth }
// tools-clipboard-impl
pub struct ClipboardTools; impl ClipboardTools { pub fn new(deps: ClipboardToolsDeps /* broker, clipboard: ClipboardPort, env, config, bus */) -> Self }
```
Zdolność do czasu rodziny `clipboard.*` w Brokerze: `gui.control(clipboard.exe)` (pseudo-aplikacja schowka; L3 → pytanie, L4 bez pytania; nie obejmuje innych aplikacji). Cofnięcie z konfliktem (schowek zmieniony po zapisie agentki) jest wstrzymane.

Historia schowka (właściciel, paleta `Ctrl+K`) — szkic, F3+:
```rust
pub struct ClipItem { pub id: ClipId, pub formats: Vec<ClipFormat>, pub ts: Timestamp, pub source_app: Option<String>, pub pinned: bool, pub excluded_reason: Option<ExcludeReason> }
pub trait ClipboardHistory { fn history(&self) -> Vec<ClipItem>; fn watch(&self) -> Subscription<ClipItem>; fn pin(&self, id); fn clear(&self); }
```

## Zależności
`core-bus/config/log-contract`, `platform-windows-contract` (`ClipboardPort`, `watch`, `IsPassword` z UIA okna źródłowego — od F6, wcześniej deny-lista aplikacji), `safety-broker-contract`, `undo-journal-contract` (poprzednia zawartość przy `Set`), `search-contract` (opcjonalne wyszukiwanie w historii — tylko Owner).

## Niezmienniki
- Wpisy z pól haseł (UIA `IsPassword`, gdy dostępne) i z aplikacji/okien z deny-listy nigdy nie trafiają do historii ani do agentek.
- Zawartość schowka to niezaufane wejście dla agentki (taint), jeśli pochodzi spoza Alfy.
- `Set` przez agentkę zapisuje poprzednią zawartość do dziennika cofania; toast „Cofnij".
- Historia szyfrowana w spoczynku, retencja domyślnie 7 dni, limit wpisów; `Clear` natychmiastowe.
- Agentka bez zgody Brokera (`gui.control(clipboard.exe)` do czasu rodziny `clipboard.*`) nie odczytuje ani nie zapisuje schowka; historia dostępna tylko dla właściciela (UI).
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
Krok w wątku i Replay z „Cofnij” (token `"<sesja>:c<id>"` → `turns_undo_step`; sesja musi być właścicielką kroku; konflikt → czytelny błąd), sekcja „Historia schowka” w palecie `Ctrl+K` (szkic), `Ctrl+Shift+V` wklej jako tekst, Ustawienia → Komputer → Schowek.

## Integracja w aplikacji
`app-agents::AgentTools` składa `ClipboardTools` nad `ClipboardPort` z `platform-windows`; cofnięcie kroku schowka obsługuje `AgentTools::undo_clipboard` (rdzeń pamięta, które identyfikatory należą do której sesji).

## Testy akceptacyjne
- `ACC-F3-tools-clipboard-01`: wpis z aplikacji na deny-liście / pola hasła (fixture UIA) → 0 wpisów w historii (100/100).
- `ACC-F3-tools-clipboard-02`: `Set` przez agentkę → „Cofnij" przywraca poprzednią zawartość (formaty: tekst, HTML, obraz, pliki).
- `ACC-F4-tools-clipboard-03`: narzędzie przez serwer MCP v0 działa dla mostu CLI z tokenem sesyjnym; bez tokenu = odmowa.

## Fake
`tools-clipboard-fake`: schowek i historia w pamięci (`platform-windows-fake`), skryptowane zmiany z aplikacji źródłowych.

## Otwarte pytania
- Agentka widzi tylko bieżącą zawartość (v0) — historia wyłącznie dla właściciela.
- Rodzina zdolności `clipboard.read/write` w katalogu Brokera (zamiast `gui.control(clipboard.exe)`) — SPEC v1 `safety-broker`.
