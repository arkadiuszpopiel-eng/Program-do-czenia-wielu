# undo-journal-impl

Dziennik cofania — usługa (`UndoService`): rdzeń z kontraktu nad trwałym magazynem `DirStore`
(`undo-store/blobs/<sha256>` z zapisem atomowym i weryfikacją hasha przy odczycie, `journal.ndjson`
append-only z `sync_data`; urwana ostatnia linia po awarii pomijana, uszkodzenie w środku = błąd),
zdarzenia `undo.*` (kolejka `flush_events`), `Module` + `module.toml`. Uszkodzony pre-image daje raport
częściowy (`UndoError::Partial`) — nigdy cicho. Szyfrowanie pre-image kluczem sesji (crypto-shredding)
— poza częścią 1. Testy: kontrakt na dysku, restart, urwany zapis, limit magazynu, 200 losowych
sekwencji z cofaniem po restarcie.
