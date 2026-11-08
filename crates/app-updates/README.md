# app-updates

Aktualizacje w aplikacji (kategoria `app-*`, korzeń kompozycji): `UpdatesApp` składa
`updater-impl::UpdateService` z ustawieniami (`updates.channel`, `updates.mode`, `updates.whats_new`) i zdarzeniem
`UpdateStatus` dla UI. Komendy `updates_*` (COMMANDS.md) deleguje `app-core` (`commands/work.rs`).

- Tryby: **automatycznie** (sprawdza raz na dobę, pobiera i przygotowuje), **pytaj** (domyślny: sprawdza, pyta przed
  pobraniem), **ręcznie** (tylko „Sprawdź teraz”). Pobieranie w tle z wznawianiem; postęp w zdarzeniach.
- Restart: `alfa.exe --alfa-restart` (launcher czeka na zwolnienie blokady instancji `running.lock`) + zamknięcie
  przez powłokę (`ShellPort::exit_app`). Odmowa w trakcie zadania agentki albo rozmowy głosowej (sonda z `app-core`).
- `mark_good` po zdrowym starcie (`Schedule::healthy_after`), „Przywróć poprzednią wersję”, „Co nowego” raz po
  aktualizacji (notatki z podpisanej paczki; wyłączalne), „O programie” (wersja, kanał, data kompilacji
  `ALFA_BUILD_DATE`, commit `ALFA_BUILD_COMMIT`, licencje z `data/licenses.json` — generuje
  `apps/desktop/scripts/gen-licenses.mjs`; w wydaniu workflow regeneruje plik przed budową).
- Bez adresu wydań i klucza minisign wbudowanych w wydanie aktualizacje są wyłączone (stan `disabled`).

Testy: prawdziwy moduł plików na katalogu tymczasowym, `updater-fake::FakeFeed`, paczki ZIP podpisane parą kluczy
minisign z testu (tryby, zdarzenia, restart z blokadą, rollback, „Co nowego”, `mark_good`).
