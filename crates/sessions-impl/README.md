# sessions-impl

Implementacja modułu `sessions` na SQLCipher (ADR 0006, ADR 0008).

- `index.db` (klucz `alfa/sessions/index`) — katalog: metadane (JSON) + liczniki (tury, nieprzeczytane,
  ostatnia tura). `<id>.db` — osobna baza każdej sesji z własnym kluczem (`alfa/sessions/<id>`), identyfikator
  UUIDv7.
- Historia: `turns(id, parent_id, branch_id, role, created_at, body)` — `body` to kanoniczny JSON niezmiennej
  części tury; `branches`, `turn_heard` (fakt append-only), `turn_hidden` (flaga widoku), `session_state`
  (aktywny liść, szkic). **Wyzwalacze odrzucają `UPDATE`/`DELETE`** na `turns`, `branches`, `turn_heard`.
- Indeksowanie tur przez `search_contract::TxIndexer` (`with_indexer`) **w tej samej transakcji** co zapis
  tury; błąd indeksu wycofuje turę.
- `delete_session`: klucz z sejfu → zamknięcie → pliki (`-wal`/`-shm`) → wpis katalogu; osierocone pliki
  sprząta `sweep_orphans` przy otwarciu.
- `SessionDbProvider`: jedna instancja `Db` na plik (blokada `open` → `index`, nigdy odwrotnie),
  `cache_size` 256 KiB na bazę (budżet RAM).
- `Module` (+ `module.toml`): zdarzenia `session.*` przez kolejkę → magistralę, bez treści tur.
- Feature `dev-file-vault`: `FileKeyVault` — klucze jawnie w plikach, **tylko testy/dev**.

Testy: kontrakt (16) + property-based z porównaniem surowych bajtów `body`, wyzwalacze, szyfrowanie i testy
szpiegowskie, 3 sesje równolegle, crypto-shredding, trwałość po ponownym otwarciu, atomowość indeksu,
zdarzenia, budżet (1000 tur < 1 s, projekcja < 50 ms).
