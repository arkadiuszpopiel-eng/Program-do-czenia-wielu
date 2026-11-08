# spike-data

Spike (i) z docs/PLAN.md §16.2 (F0) dla ADR 0008: czy w **jednym pliku** SQLite da się połączyć
szyfrowanie SQLCipher (`rusqlite` z `bundled-sqlcipher-vendored-openssl`), wektory `sqlite-vec`
(zarejestrowane statycznie przez `sqlite3_auto_extension`) i FTS5 (w bundlu SQLCipher).

- `src/lib.rs` — otwieranie bazy z kluczem, schemat (`messages`, `vec0`, FTS5), wstawianie, kNN, FTS.
- `src/main.rs` — pomiar: rozmiar pliku, czasy 100 wstawień i zapytań (`cargo run -p spike-data`).
- `tests/spike.rs` — dowody: kNN, FTS5 po polsku, otwarcie bez klucza / ze złym kluczem = błąd,
  nagłówek pliku nie jest jawnym SQLite, crypto-shredding per sesja (osobny plik).

Wynik i rekomendacja: `evals/spikes/i-dane/RESULT.md`. Crate tymczasowy — nie jest modułem
(brak trójki `-contract/-impl/-fake`); do usunięcia po przeniesieniu wniosków do ADR 0008.
