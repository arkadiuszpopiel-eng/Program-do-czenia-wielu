# undo-journal-contract

Kontrakt i rdzeń dziennika cofania (docs/modules/undo-journal/SPEC.md, PLAN §8.7, §14.8). `Journal`
wykonuje operacje `fs.*` (zapis, kopia, przeniesienie, Kosz, trwałe usunięcie) przez `FsPort`:
pre-image → operacja (token `UndoToken`) → wpis append-only; błąd wpisu cofa operację (brak wpisu =
brak operacji). Kroki grupują operacje („Delta: przeniesiono 14 plików”, polska odmiana). Cofnięcie
najpierw sprawdza konflikty całego kroku (plik zmieniony później → `UndoError::Conflict`, nic nie jest
ruszane), potem odtwarza w odwrotnej kolejności: token platformy z tego uruchomienia albo pre-image
(po restarcie). Snapshot zakresu dla shella: kopia plików z limitem (prostsze niż shadow-git), stan
„po” zapisywany przy zatwierdzeniu. Limity: pre-image/plik, magazyn (wypieranie najstarszych kroków),
retencja. Trait `UndoJournal`, magazyn `JournalStore` + `MemStore`. Testy: 3 × 256 losowych sekwencji
(tokeny platformy, wyłącznie pre-image, restart) → 100% przywrócenia; testy kontraktowe (`contract-tests`).
