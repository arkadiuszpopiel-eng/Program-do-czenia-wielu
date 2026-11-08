# transfer-fake

Atrapa modułu `transfer`: `FakeTransfer` używa tego samego silnika co `transfer-impl`, ale paczki trzyma w pamięci
(klucz: ścieżka), „szyfrowanie” to skrót hasła, zegar jest wirtualny (`VirtualClock`), identyfikatory kopii
deterministyczne (`SeqIds`). Dodatkowo: `fail_next` (skryptowane błędy), rejestr zdarzeń, `MemoryDocumentStore`
oraz porty zawodzące po N zapisach (`FailureBudget`, `FlakySessions`) do testów „import przerwany w połowie”.
Testy: kontrakt współdzielony + property-based `ACC-F1-transfer-03` (przerwanie w dowolnym punkcie → spójne drzewa,
rollback przywraca stan sprzed importu). Wolno używać wyłącznie w `dev-dependencies` innych modułów.
