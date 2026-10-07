# triggers-impl

Moduł `triggers`: `TriggersModule` (`Triggers` + `Module`) — timer tokio, wejścia z magistrali
(`session.turn.appended`, `scheduler.task.finished` z pochodzeniem i taintem z ładunku), wejście
obserwatora plików (`file_created` + port `FileWatchPort`), zadania do `Arc<dyn Scheduler>`, zdarzenia
`triggers.*`, stan w `FileTriggerStore`. Testy: kontrakt, moduł (magistrala, plik, restart) oraz
**F5-04** (`tests/compliance.rs`): „most nie startuje z wyzwalacza” 0/100 na `agent-backends-fake`
z jawną zgodą tras, plus kontrole pozytywne.
