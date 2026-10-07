# updater-contract

Kontrakt modułu `updater` (docs/modules/updater/SPEC.md, ADR 0007).

- `Layout`: `%LOCALAPPDATA%\Alfa\` — `versions\<ver>\alfa-desktop.exe`, stały `alfa.exe`, `current.json`,
  stały `webview-data\` (poza katalogami wersji).
- `CurrentState` (`current.json`): aktywna, poprzednia, `pending` (czeka na `mark_good`), licznik szybkich awarii,
  wersje wycofane. Logika wspólna `-impl`/`-fake`: `choose_version` (aktywna → poprzednia → błąd; bez stanu —
  najnowsza poprawna), `switched`/`rolled_back`/`marked_good`, `decide_exit` (`CrashPolicy`: wyjście ≠ 0 w oknie
  15 s = szybka awaria; wersja `pending` — od razu powrót, dobra — po 2 awariach), `prune_victims` (zostaje N,
  zawsze aktywna, poprzednia i przygotowane nowsze).
- `Release`/`ReleaseManifest` (wersja, sha256, podpis minisign, „Co nowego”, `min_previous`), `select_update`,
  wiązanie podpisu z wersją (`version:<ver>` w komentarzu zaufanym).
- Trait `Updater`; zdarzenia `updater.*`; feature `contract-tests` (6 przypadków, w tym dobre/złe podpisy).
- F3: `Channel`/`UpdateMode`/`InstallIntent` (`check_install_allowed` — wersja ≤ bieżącej tylko jako jawny rollback),
  port `ReleaseFeed` (manifest + pobieranie z wznawianiem), adresy tylko `https://` (względne wobec katalogu
  manifestów), `validate_package_path` + `PackageLimits` (reguły ZIP), `UpdateStatus`/`UpdatePhase`, `UpdatesFile`
  (`updates.json`), `AppExit::Unconfirmed` + `CrashPolicy::confirm_ms` (brak `mark_good` = awaria nowej wersji).
