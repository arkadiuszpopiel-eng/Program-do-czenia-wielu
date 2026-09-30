# ADR 0008 — Dane: SQLite (WAL) szyfrowane + sqlite-vec + FTS5 w jednej bazie per sesja

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany (zgodność SQLCipher + sqlite-vec + FTS5 — spike (i) w F0) |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §1.2 (Dane), §10, §11, §13, §15.1, §16.2 (F0 spike i, F1, F7), §17 |

## Kontekst

Każdy czat ma osobną pamięć (zakres `sesja` domyślny); pamięć semantyczna wymaga wektorów i wyszukiwania pełnotekstowego; logi mają być usuwalne przez crypto-shredding bez łamania łańcucha hashy; `forget` musi działać kaskadowo (embeddingi, streszczenia, kopie, eksporty). Program ma być lekki — jeden silnik zamiast bazy + osobnego indeksu wektorowego.

## Decyzja

| Element | Wybór |
|---|---|
| Silnik | SQLite w trybie WAL |
| Szyfrowanie w spoczynku | SQLCipher (zgodność z sqlite-vec sprawdzana w F0, spike i) |
| Wektory | rozszerzenie sqlite-vec w tej samej bazie |
| Pełny tekst | FTS5 w tej samej bazie |
| Granica | **jedna szyfrowana baza per sesja**; usunięcie klucza sesji kasuje także indeks wektorowy i FTS |
| Klucze | per sesja, w Credential Manager/DPAPI (per maszyna); payloady logów szyfrowane kluczem sesji (crypto-shredding) |
| Logi | append-only NDJSON + indeks SQLite (§13); schematy wersjonowane, upcastery, testy migracji |
| Warstwy pamięci | robocza (RAM + checkpoint), epizodyczna (SQLite), semantyczna (SQLite szyfrowane + sqlite-vec + FTS5), proceduralna (pliki + indeks) |
| Zakresy | `sesja` · `projekt` · `globalna` · `agentka`; dzielenie tylko jawnie; wpisy z niezaufanej treści nie awansują do globalnej |

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Osobna baza wektorowa (Qdrant, LanceDB) | drugi silnik, drugi proces lub duża biblioteka; `forget` kaskadowo trudniejszy; przeczy lekkości |
| Jedna wspólna baza dla wszystkich sesji | izolacja sesji byłaby logiczna, nie kryptograficzna; usunięcie sesji nie kasowałoby indeksu; testy szpiegowskie trudniejsze |
| Pliki JSON/Markdown jako pamięć | brak FTS i wektorów; brak transakcji; brak szyfrowania |
| Szyfrowanie na poziomie systemu plików (EFS/BitLocker) | nie daje klucza per sesja → brak crypto-shredding i kaskadowego `forget` |
| Zewnętrzne embeddingi jako jedyne | pamięć musi działać offline i lokalnie; embeddingi lokalne wielojęzyczne (jakość PL do weryfikacji) |

## Konsekwencje

- Moduły `sessions`, `memory`, `search`, `artifacts`, `core-log` dzielą ten sam schemat i mechanizm kluczy.
- Wyszukiwanie między sesjami jest funkcją UI jako użytkownik (`Ctrl+Shift+F`), nigdy narzędziem agentki — otwiera wiele baz.
- Kryteria: F1 — ≥ 3 sesje równolegle, 0 przecieków między sesjami (testy szpiegowskie); F7 — recall@5 ≥ 0,85 na ≥ 200 zapytaniach PL, kaskada `forget` zweryfikowana.
- Paczka `.alfa` (`transfer`) eksportuje wybrane sesje i zakresy pamięci; sekrety i klucze sesji nie wchodzą do zwykłego eksportu.
- Ryzyko: SQLCipher a rozszerzenia ładowane dynamicznie (sqlite-vec) — jeśli spike (i) wykaże niezgodność, patrz „Jak cofnąć".
- Retencja logów Narzędzia/GUI domyślnie 7 dni; limity dysku do ustalenia w F0.

## Jak cofnąć

- Niezgodność SQLCipher + sqlite-vec: wariant zapasowy to szyfrowanie payloadów na poziomie aplikacji (kolumny szyfrowane kluczem sesji, indeks wektorowy na zaszyfrowanych blobach niemożliwy → sqlite-vec na osobnym pliku per sesja, także szyfrowanym) — decyzja po spike'u (i), aktualizacja tego ADR.
- Przejście na osobną bazę wektorową jest możliwe za kontraktem `search-contract`; koszt: drugi silnik i ręczna kaskada `forget`.

## Wynik spike'u (i) — 2026-09-30 (`crates/spike-data`, `evals/spikes/i-dane/RESULT.md`)

**Potwierdzone:** SQLCipher 4.14 (rusqlite `bundled-sqlcipher-vendored-openssl`, SQLite 3.51.3) + sqlite-vec 0.1.9 (statycznie, `sqlite3_auto_extension`) + FTS5 (w bundlu, `bundled-full` niepotrzebne) działają w jednej szyfrowanej bazie; crypto-shredding (dwa pliki, dwa klucze, klucz A nie otwiera B) i zły/brak klucza → `file is not a database`. Wariant zapasowy z „Jak cofnąć" **nie jest potrzebny**.

Uzupełnienia decyzji:
- **Klucze surowe 32 B** (`x'…'`) z Credential Manager, bez KDF — otwarcie 67 µs vs 142 ms z hasłem (PBKDF2 256k).
- `PRAGMA cipher_log_level = NONE` — SQLCipher pisze na stderr przy złym kluczu.
- Kaskada `forget`/usunięcia sesji kasuje także pliki `-wal` i `-shm`.
- FTS5 `unicode61` nie składa „ł" (żółć↔zółc tak, zolc nie) → normalizacja polskich znaków w module `search`.
- Crate danych wymaga jednego `unsafe` (rejestracja rozszerzenia) → wyjątek od `unsafe_code = forbid` tylko dla tego crate'a, z komentarzem.
- Build: OpenSSL ze źródeł (+1–2 min czystego builda, +~7 MB binarki); na Windows/MSVC wymaga Perla (Strawberry) — jest na `windows-latest`, na self-hosted runnerach do zainstalowania.
- sqlite-vec = kNN brute-force; wydajność przy realnych wymiarach embeddingów mierzona w F7.
