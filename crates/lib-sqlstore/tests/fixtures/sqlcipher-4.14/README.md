# Publiczna fixture SQLCipher4.14 / SQLite3.51.3

Syntetyczny zapis ze starego silnika i aktywny WAL, zachowane przed zamknięciem połączenia.
Nie zawiera danych użytkownika. Publiczny klucz testu to32 bajty42 (`fixture_key`); nie jest sekretem.
Nowy silnik otwiera wyłącznie tymczasową kopię obu plików.

SHA256:

- database.fixture: 4f46d80e0cbadd9e2ba2edfdddf9e16034dffe1d743350eb936dda05fa6a5ca0
- wal.fixture: f869f42133e9fbbb34cf668a6cf8014e356da59f63cde09bd3f3bd3268887d0b

Binary capture SHA256: 00cd43122be2f3c3186696b879b4ceee3f83e4d8a4f163980bf9e2c8d083141e

Odtworzenie: ręczny test `capture_legacy_encrypted_wal_fixture` na starej wersji,
z `ALFA_SQLCIPHER_FIXTURE_DIR` wskazującym nowy pusty katalog. Nowy silnik ma odrzucić
próbę stworzenia fixture starego silnika.
