# tools-uia — SPEC (v1: narzędzia zaimplementowane, F5/F6)

## Cel
Narzędzia agentek UI Automation: drzewo elementów okna, wyszukiwanie, tekst (`TextPattern`, tylko odczyt) i akcje **wyłącznie przez wzorce** (bez wejścia syntetycznego). Trasa „UI Automation” z PLAN §7.1 (przed wizją i wejściem syntetycznym); drzewo ubogie → podpowiedź trasy wizji.

## Fala i priorytet
F5 (v1.5: `TextPattern` do odczytu) + F6 (v2: akcje), P1.

## Kontrakt
```rust
uia_tree      { window, max_depth?, max_nodes?, include_offscreen? } → TreeOutput { window, nodes: [NodeOut], truncated, sparse }
uia_find      { window, name?, name_contains?, role?, automation_id?, class_name?, max_results? } → FindOutput
uia_read_text { element, max_chars? }                                → TextOutput { element, text, truncated }
uia_act       { element, action: invoke|set_value|toggle|expand|collapse|select|scroll, value?, direction?, amount? }
              → ActOutput { element, action, verified, after }
// NodeOut { element: "w<okno>:<RuntimeId>", depth, role, name, automation_id, value?, password, enabled, focused,
//           toggle?, expand?, selected?, x, y, width, height, patterns }
pub struct UiaTools; impl UiaTools { pub fn new(UiaToolsDeps { desktop, uia: UiaPort, broker, config, bus }) }
```
Zdarzenia: `tool.uia.read` (narzędzie, okno, liczba węzłów), `tool.uia.act` (okno, akcja, rola), `tool.gui.verify`.

## Zależności
`tools-window-contract` (bramka GUI), `tools-common-contract`, `safety-broker-contract`, `platform-contract` (`UiaPort`, `ElementRef`, `UiaAction`), `core-bus-contract`.

## Niezmienniki
- Odczyty to **niezaufana treść**: wynik `untrusted = Screen`, taint sesji zgłoszony Brokerowi, sekrety redagowane (nazwy, wartości, tekst).
- Wartość pola hasła (`IsPassword`) nigdy nie wychodzi z portu; `uia_read_text` na polu hasła = odmowa; `set_value` w pole hasła = odmowa (agentka nie zna haseł).
- Okna i elementy procesów chronionych: odmowa przed Brokerem; port sprawdza właściciela okna i proces elementu (fail-closed dla procesu nieznanego); odwołanie „podrobione” (RuntimeId z innego okna) nie rozwiązuje się.
- Akcja tylko przez wzorzec obsługiwany przez element, element włączony; stan przed i po → `verified` + zdarzenie weryfikacji (F6-04).
- Limity czasu UIA (wywołanie 5 s, drzewo 15 s) → `Timeout` z podpowiedzią trasy wizji.

## Zdolności / uprawnienia
`gui.control(<aplikacja okna>)`; odczyty z faktem „dane prywatne” (trifecta). Odwracalność: odczyty `yes`, `uia_act` **`no`** (wyższe ryzyko w klasyfikatorze).

## Izolacja
`inproc`, `lazy`; port UIA na własnym wątku MTA (`platform-windows-gui-impl`).

## Budżet zasobów
RAM ≤ 4 MB (+ drzewo ≤ 1000 węzłów); wynik dla modelu ≤ 30 000 znaków.

## Konfiguracja (klucze TOML)
`[tools.uia] max_nodes = 300`, `text_max_chars = 20000`, `output_max_chars = 30000`.

## Wkład do UI
Kroki w wątku/Replay; ramki elementów w panelu „Ekran” (F6, inna sesja) ze zdarzeń.

## Testy akceptacyjne
- `ACC-F6-tools-uia-01`: 0 akcji w oknach Alfy/Brokera w 200 losowych próbach (property, `tests/uia.rs`).
- `ACC-F6-tools-uia-02`: taint po odczycie (sesja `tainted`), redakcja sekretów, brak wartości haseł.
- `ACC-F5-tools-uia-03`: `TextPattern` czyta tekst dokumentu w 5 aplikacjach (F5-11, self-hosted).
- F6-03: drzewo ubogie wykryte (`sparse`) → trasa wizji.

## Fake
`tools-uia-fake`: prawdziwe manifesty i walidacja; testy impl na `platform-fake::FakeDesktop` (drzewa elementów, zawieszenie UIA).

## Otwarte pytania
- `LegacyIAccessiblePattern`/`Window`/`RangeValue` — po macierzy aplikacji (F6-03).
- Zdarzenia UIA (zmiana fokusu/struktury) zamiast odpytywania — SPEC v2.
