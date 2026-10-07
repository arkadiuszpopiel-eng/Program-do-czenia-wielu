# tools-office — SPEC (v1: narzędzia zaimplementowane, F6)

## Cel
Narzędzia agentek dla Worda i Excela przez automatyzację COM (trasa „COM” z PLAN §7.1 przed UIA i wizją): odczyt tekstu, tabel i zakresów oraz **edycja kopii roboczej zapisywanej jako nowa wersja** — oryginał nigdy nie jest zmieniany. Nie obsługuje makr, szablonów ani innych aplikacji Office (P2).

## Fala i priorytet
F6, P1 (Word/Excel); PowerPoint/Outlook — P2.

## Kontrakt
```rust
office_read { path, what?: text|tables|range|sheets, sheet?, range?, formulas?, max_chars? }
            → ReadOutput { path, app, text?, tables, cells, sheets, truncated, protected_view, macros_present }
office_edit { path, edits: [insert_text{position,text} | replace_text{find,replace,all?} | insert_table{position,rows}
                           | set_cells{sheet?,start,rows:[[number|bool|"tekst"|"=formuła"|null]]} | add_sheet{name}],
              output? }  → EditOutput { original, output, applied, replacements, bytes, undo_step }
pub struct OfficeTools; impl OfficeTools { pub fn new(OfficeToolsDeps { fs, office: OfficePort, journal, broker, env, deny, config, bus }) }
```
Zdarzenia: `tool.office.read` (aplikacja, Protected View — bez treści), `tool.office.edit` (liczba edycji, krok cofania).

## Zależności
`tools-common-contract`, `platform-apps-contract` (`OfficePort`), `platform-contract` (`FsPort`), `undo-journal-contract`, `safety-broker-contract`, `compliance-contract`, `core-bus-contract`.

## Niezmienniki
- Ścieżki: postać i deny-lista (poświadczenia, profile) **przed** Brokerem; nowa wersja nigdy nie jest oryginałem i ma format tej samej aplikacji; domyślnie `<nazwa> (Alfa).<ext>`, potem `(Alfa 2)`…
- Bajty oryginału przez `FsPort`, przetwarzanie na kopii w porcie; nowa wersja **tylko** przez `undo-journal` (pre-image: brak pliku albo poprzednia zawartość) → „Cofnij” usuwa ją lub przywraca stan.
- Makra zawsze wyłączone: wynik portu bez `AutomationSecurity = 3` → odmowa i brak zapisu (fail-closed).
- Plik z Internetu (MOTW): odczyt w Protected View, edycja → odmowa „Protected View”.
- Formuły tylko z listy dozwolonej (walidacja w narzędziu i w porcie); napis od `'` = tekst dosłowny.
- Odczyty to **treść niezaufana** (`untrusted = File`, taint sesji zgłoszony Brokerowi), sekrety redagowane; tokeny jednorazowe zwalniane po akcji.

## Zdolności / uprawnienia
`office_read`: `fs.read(dokument)` + `gui.control(winword.exe|excel.exe)` (fakt „dane prywatne”); `office_edit`: dodatkowo `fs.write(nowa wersja)`. Odwracalność `yes` (nowa wersja w dzienniku). Grupy ról: `office`, `office.read`, `office.write`.

## Izolacja
`inproc`, `lazy`; port Office na wątku STA (`platform-windows-office-impl`), wywołania przez `spawn_blocking`.

## Budżet zasobów
RAM ≤ 8 MB (+ dokument ≤ 64 MiB w pamięci na czas operacji); wynik dla modelu ≤ 40 000 znaków.

## Konfiguracja (klucze TOML)
`[tools.office] output_max_chars = 40000`, `text_max_chars = 30000`, `table_max_cells = 2000`; `[platform.office] work_dir`, `call_timeout_ms = 60000`.

## Testy akceptacyjne
- `ACC-F6-tools-office-01`: edycja kopii → nowa wersja, oryginał bajt w bajt bez zmian; cofnięcie usuwa nową wersję / przywraca nadpisany plik (`tests/office.rs`).
- `ACC-F6-tools-office-02`: makra nigdy nie uruchomione; port bez wyłączonych makr → odmowa, brak zapisu.
- `ACC-F6-tools-office-03`: Protected View, deny-lista, odmowa Brokera przed otwarciem Office, formuły DDE/sieciowe odrzucone.
- F6-01 (kategoria „Office”): zestaw zadań w VM — self-hosted.

## Fake
`tools-office-fake` (manifesty i walidacja z kontraktu, wyniki skryptowane); testy impl na `platform-apps-fake::FakeOffice` + `platform-fake::FakeFs`.

## Otwarte pytania
- Excel zapisuje tryb obliczeń ręczny w nowej wersji (bezpieczeństwo ponad wygodę) — wartości formuł przeliczą się po F9; do decyzji właściciela.
- Zawieszony proces Office po limicie czasu — zabijanie po PID (Diagnosta).
