# transfer-impl

Implementacja modułu `transfer` (`ZipTransfer`) + `module.toml`.

- **Kontener** (`container`): ZIP, `manifest.json` pierwszy i nieskompresowany, reszta deflate; zapis przez archiwum
  pośrednie i kopiowanie surowe, atomowa zamiana pliku (pliki tymczasowe sprzątane). Odczyt: limity (`Limits`: liczba
  wpisów, rozmiar wpisu/łączny/manifestu, stopień kompresji), reguła ścieżek, brak dowiązań/katalogów/wpisów
  szyfrowanych ZIP, zgodność listy z manifestem w obie strony, SHA-256 i rozmiar przy każdym odczycie.
- **Szyfrowanie** (`crypto`): `ALFAENC1` + nagłówek JSON (AAD) + XChaCha20-Poly1305 w konstrukcji STREAM
  (fragmenty 64 KiB, nonce = prefiks ‖ licznik BE32 ‖ flaga ostatniego). Klucz: Argon2id (46 MiB, t=2, p=1; sól 16 B)
  z hasła albo klucz maszyny z Credential Managera (`transfer/snapshot-key`) dla snapshotów. Odczyt losowy
  (`DecryptingReader: Read + Seek`) — ZIP czytany wprost z szyfrogramu, bez jawnej kopii na dysku.
- **Snapshoty** przed importem w `snapshots_dir` (rotacja 5), rollback = przywrócenie zapisanych + usunięcie utworzonych.
- **Kopie zapasowe**: `backup` = eksport `kind = backup` do katalogu + rotacja N ostatnich (`alfa-backup-<znacznik>.alfa`).
- **`DirDocumentStore`**: magazyn dokumentów na katalogu z filtrem rozszerzeń, zapisem atomowym i ochroną przed
  wyjściem poza korzeń (także przez dowiązania).
- Zdarzenia Audytu na magistralę (`transfer.export.*`, `transfer.import.*`, `transfer.rolled_back`, `transfer.backup.completed`)
  bez treści i bez pełnych ścieżek.

Testy: kontrakt współdzielony na prawdziwym ZIP, odporność (uszkodzony/obcięty plik, zła suma, path traversal z
property-testem nazw wpisów, zip-bomb), kryptografia (granice fragmentów, losowy dostęp, manipulacje, obcięcie),
migracja v0 → v1 na syntetycznej paczce, zdarzenia, budżet czasu (`ALFA_PERF_BUDGETS=1` — ścisły).
