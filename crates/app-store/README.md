# app-store

Tabele aplikacji w szyfrowanych bazach sesji (kategoria `app-*`, wydzielona z `app-core` — limit
rozmiaru crate'a): fakty o turach spoza kontraktu `sessions` (`TurnMeta`), log stanów, oceny, oś czasu
v0, katalog roboczy sesji, nagłówki i kroki przebiegów agentek (Replay, także podprzebiegi).
Wyłącznie dopisywane (historia append-only); migracje w przestrzeni nazw `app-core` (zgodność baz).
Testy: przez `crates/app-core/tests/*`.

Kandydat #58: `execution_records` projektuje tylko wcześniejsze tury wybranej gałęzi
do maks.1536bajtów JSON/4ostatnich wpisów i liczników całego wyboru. Odczyt w jednej
transakcji; źródłem są TurnMeta i ostatnie nagłówki/kroki Replay oraz globalne Undo.
Braki są jawne, CLI/konflikt/legacyDone+intent nie poświadczają wykonania. Done jest
zapisanym statusem, nie dowodem skutku lub testu. Nazwy tylko z manifestów, bez ID,
etykiet, celów, argumentów, intencji, ścieżek i wyjść. Pusty wybór nie czyta bazy.
To dane aplikacji; nie syntetyzuje wywołań ToolUse ani odpowiedzi modelu.
Przed użyciem: regresje magazynu, rzeczywistego żądania i próba modelu; sam getter
nie usuwa halucynacji. Przyrost lokalny, nie deklaracja odbioru.
