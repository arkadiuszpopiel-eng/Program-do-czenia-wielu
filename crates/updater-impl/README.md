# updater-impl

Implementacja modułu `updater` (`FsUpdater`) + `module.toml` + **binarium `alfa`** (stały launcher
`%LOCALAPPDATA%\Alfa\alfa.exe`).

- Pliki: `current.json` czytany odpornie (brak/uszkodzony/nieznany schemat → najnowsza poprawna wersja), zapisywany
  atomowo (tymczasowy + `fsync` + `rename`); katalog wersji poprawny, gdy nazwa to kanoniczny semver, jest
  `alfa-desktop.exe`, a opcjonalny `version.json` zgadza się z nazwą. Sprzątanie usuwa tylko katalogi wersji.
- Weryfikacja wydań: SHA-256 + minisign (`minisign-verify`, strumieniowo, tylko podpisy „prehashed”), klucz publiczny
  z konfiguracji, wymagany `version:<ver>` w komentarzu zaufanym (ochrona przed podsunięciem starszej paczki).
- **Launcher** (`launcher`): czyta stan, uruchamia `versions\<ver>\alfa-desktop.exe` z argumentami bez zmian
  (`alfa://…`, ścieżki „Otwórz w Alfie”/„Wyślij do”), obserwuje okno crash-loop i ponawia/wraca do poprzedniej wersji;
  błędy do `launcher.log` (brak konsoli: `windows_subsystem = "windows"`). Binarium jest w tym pakiecie, bo osobny
  pakiet nie może zależeć od `-impl` (scripts/check-deps.sh).
- **F3**: `HttpFeed` (manifest kanału i paczka przez HTTPS, wznawianie `Range`, przekierowania tylko na https, bez
  telemetrii), `install` (rozpakowanie ZIP do `versions\.staging-<ver>` → `rename`; ochrona przed path traversal,
  strumieniami NTFS, nazwami urządzeń, dowiązaniami, duplikatami i zip-bomb), `UpdateService` (sprawdź → pobierz →
  SHA-256 + minisign → rozpakuj → `activate_staged` → sprzątanie, anulowanie i wznowienie, „Co nowego”, rollback
  użytkownika), `selfupdate` (nowy `alfa.exe` obok, samotest, zamiana przy starcie), `instance` (blokada instancji dla
  restartu), `entry` (tryby `--alfa-restart`, `--alfa-installed <ver>`, `--alfa-launcher-check`), `WatchdogSignal`
  (rollback zlecony przez watchdoga). Launcher obserwuje nową wersję do `mark_good` (≤ 5 min) — awaria albo
  zawieszenie = powrót do poprzedniej.

**Moduł Jądra — zmiany wymagają przeglądu człowieka (AGENTS.md).** Testy: kontrakt (prawdziwe pary kluczy minisign
generowane w teście), wektory z dokumentacji minisign, launcher na atrapach i na prawdziwych procesach (Unix), pełny
cykl na lokalnym serwerze HTTP (`tests/update_flow.rs`, `tests/update_safety.rs`, `tests/launcher_modes.rs`).
