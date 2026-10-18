# Pamięć projektu Alfa

Kopia wszystkiego, co asystent i właściciel zapisują o projekcie poza kodem: stan prac, ustalenia, zasady
pracy i dziennik sesji. Leży w repozytorium, żeby wędrowała razem z projektem (zasada właściciela
z 2026-10-12). **To kopia** — pliki źródłowe są tam, gdzie czytają je narzędzia; odświeża ją
`bash scripts/memory-sync.sh` na końcu każdej sesji. Data ostatniej kopii: `OSTATNIA-KOPIA.txt`.

| Katalog              | Co zawiera                                                                                                                                                                         | Źródło                                                  |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| `1-stan-projektu/`   | `status.md` — bieżący stan i wersja; `handoff.json` — przekazanie między sesjami (ostatni commit, zadania na następną sesję, wyniki testów); `decisions.md` — decyzje właściciela | `../memory/` obok repozytorium                          |
| `2-zasady-pracy/`    | Zasady, które właściciel ustalił z asystentem (budowanie, testowanie, pobrania, głos, przeglądy interfejsu); `MEMORY.md` to spis treści                                            | `~/.claude/projects/<projekt>/memory/` na komputerze    |
| `3-dziennik-sesji/`  | Notatki z kolejnych dni pracy (`today-RRRR-MM-DD.md`), skrót ostatniego tygodnia (`recent.md`), archiwum                                                                           | `../.remember/` obok repozytorium                       |

## Od czego zacząć

1. `1-stan-projektu/status.md` — co jest zrobione, co otwarte, jak sprawdzać.
2. `1-stan-projektu/handoff.json` → `next_session.tasks` — co robić dalej.
3. `2-zasady-pracy/MEMORY.md` — zasady obowiązujące przy każdej zmianie.

Plan i historia zmian są osobno: [`../plan/`](../plan/README.md), [`../../CHANGELOG.md`](../../CHANGELOG.md).

## Zasady

- Plików w podkatalogach nie edytuj tutaj — przy następnej kopii zastąpi je wersja ze źródła. Edytuj źródło
  i uruchom `bash scripts/memory-sync.sh`.
- Skrypt dodaje nowe i aktualizuje zmienione pliki (sama zmiana końców wierszy CRLF/LF nie jest zmianą), ale
  **plików, których nie ma u źródła, nie usuwa**. Taki plik (np. zasady zapisane na drugim komputerze) zostaje
  tutaj i trafia na listę `OSTRZEŻENIE` z gotowymi poleceniami przywrócenia go do źródła (`mkdir -p …`,
  `cp -n …` — niczego u źródła nie nadpisuje). Wyjątek: notatka dziennika `today-RRRR-MM-DD.md`, którą
  wtyczka zamknęła u źródła jako `today-RRRR-MM-DD.done.md`, znika stąd od razu.
- `bash scripts/memory-sync.sh --prune` — usuwa z kopii pliki, których nie ma u źródła. Tylko wtedy, gdy
  usunięto je u źródła celowo (2026-10-17/18 lustro jako zachowanie domyślne skasowało stąd zasady pracy, gdy
  dysk przeszedł na komputer z pustą pamięcią asystenta).
- Ochrona przed nowym komputerem: gdy źródło **zasad pracy** wygląda na nowe (nie ma go, nie ma w nim spisu
  `MEMORY.md` albo brakuje w nim co najmniej połowy plików z `2-zasady-pracy/`), skrypt nie nadpisuje tam
  plików różniących się od źródła (wypisuje polecenia `diff -u`), `--prune` niczego tam nie usuwa,
  `OSTATNIA-KOPIA.txt` się nie zmienia, a skrypt kończy się kodem 3 (`WYNIK: KOPIA NIEPEŁNA`). Najpierw
  przywróć zasady poleceniami z ostrzeżenia, połącz spis `MEMORY.md` i uruchom skrypt ponownie; po kopii
  sprawdź `git status docs/pamiec-projektu`. Stan projektu i dziennik sesji są aktualizowane zawsze.
- Żadnych sekretów: plik źródłowy z czymś, co wygląda na klucz, nie trafia do kopii (kod 1, skrypt wskazuje
  plik u źródła); plik z kluczem, który już jest w kopii, zastępuje czysta wersja ze źródła, a bez niej skrypt
  podaje polecenie usunięcia go stąd.
- Test skryptu (piaskownica, niczego tu nie zmienia): `bash scripts/tests/memory-sync.test.sh`.
