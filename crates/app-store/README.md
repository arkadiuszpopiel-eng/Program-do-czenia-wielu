# app-store

Tabele aplikacji w szyfrowanych bazach sesji (kategoria `app-*`, wydzielona z `app-core` — limit
rozmiaru crate'a): fakty o turach spoza kontraktu `sessions` (`TurnMeta`), log stanów, oceny, oś czasu
v0, katalog roboczy sesji, nagłówki i kroki przebiegów agentek (Replay, także podprzebiegi).
Wyłącznie dopisywane (historia append-only); migracje w przestrzeni nazw `app-core` (zgodność baz).
Testy: przez `crates/app-core/tests/*`.
