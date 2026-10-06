# app-files

Pliki w aplikacji (kategoria `app-*`, korzeń kompozycji; PLAN §11, §14.8, §15.1). `FilesApp` obsługuje komendy
`attachments_*`, `sessions_export_conversation` i `backups_*` (COMMANDS.md), które deleguje `app-core`
(`commands/work.rs`); `app-core` woła też `prepare`/`commit` przy `turns_send` i `provider_blocks` przy budowie
historii dla modelu.

- **Załączniki composera** (`attach`): wybór (natywny dialog), upuszczenie na okno główne (ścieżki zna tylko powłoka —
  `AppCore::attachments_dropped`; UI pobiera je raz, ważne 15 s), wklejenie ze schowka systemowego (pliki albo PNG).
  Plik jest sprawdzany (zwykły plik, poza danymi Alfy, deny-listą `compliance` i katalogami poświadczeń — także przez
  dowiązania) i **kopiowany** do `…\Sesje\<nazwa>\in` przez jeden uchwyt z limitem rozmiaru. Limity: 10 plików,
  25 MB pliku, 100 MB razem. Przy wysłaniu kopie stają się artefaktami sesji (`Origin::User`), tura dostaje bloki
  `Attachment` (z SHA-256), a sesja — flagę skażenia, gdy załącznik jest tekstem albo dokumentem.
- **Dla modelu**: obraz (PNG/JPEG/GIF/WebP z poprawną sygnaturą, ≤ 5 MB) jako blok `Image` base64, tekst (≤ 100 000
  znaków) w delimitacji treści niezaufanej `tools-common`, reszta — tylko nazwa, typ i rozmiar. Treść czytana
  z artefaktu i porównywana z SHA-256 z chwili wysłania — plik zmieniony później nie podmienia historii.
- **Eksport rozmowy** (`export`): aktywna gałąź (albo jedna wiadomość) bez tur ukrytych i systemowych do Markdown albo
  samodzielnego HTML (treść przez `lib-markdown`, CSP `default-src 'none'`, fonty systemowe, tryb ciemny).
- **Kopie zapasowe** (`backup`): `Transfer::backup` do katalogu wybranego wyłącznie natywnym dialogiem, rotacja N
  najnowszych, odstęp w godzinach, nie na baterii ani przy pełnym ekranie, ponowienie po błędzie najwcześniej po
  godzinie (toast). Hasło w Credential Managerze → kopie szyfrowane i z sesjami prywatnymi. „Sprawdź” = test
  przywracania (otwarcie, sumy, odszyfrowanie, dry-run). Stan per maszyna w `state\backup.json`.
  „Przywróć…” (fala 4): `backups_restore(file)` w `app-core` — tylko nazwa z listy kopii → jednorazowy uchwyt
  `TransferPort` (15 s) dla podglądu importu; UI nie podaje ścieżki.
- **Artefakty w `.alfa`** (`docs`): magazyn dokumentów kategorii `artifacts` (`<sesja>/<artefakt>/<plik>`); import do
  `…\Sesje\Import\…` i rejestracja, gdy sesja już istnieje (inaczej uzgodnienie przy kolejnym otwarciu sesji).

Testy: atrapy sesji, artefaktów i schowka, prawdziwy `transfer-impl` (kopie, rotacja, harmonogram, sekrety, test
przywracania do świeżej instalacji z rollbackiem), deny-listy i limity, projekcja dla modelu, eksport (XSS w tytule
i treści).
