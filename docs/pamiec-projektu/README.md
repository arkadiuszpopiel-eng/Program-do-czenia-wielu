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

- Plików w podkatalogach nie edytuj tutaj — zmiana zniknie przy następnej kopii. Edytuj źródło i uruchom
  `bash scripts/memory-sync.sh`.
- Żadnych sekretów: skrypt odmawia kopii, gdy znajdzie coś, co wygląda na klucz.
