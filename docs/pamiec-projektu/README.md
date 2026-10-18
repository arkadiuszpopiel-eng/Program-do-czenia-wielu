# Pamięć projektu Alfa

Kopia wszystkiego, co asystent i właściciel zapisują o projekcie poza kodem: stan prac, ustalenia, zasady
pracy i dziennik sesji. Leży w repozytorium, żeby wędrowała razem z projektem (zasada właściciela
z 2026-10-12). **To kopia** — pliki źródłowe są tam, gdzie czytają je narzędzia; odświeża ją
`bash scripts/memory-sync.sh` na końcu każdej sesji. Data ostatniej kopii: `OSTATNIA-KOPIA.txt`.

| Katalog              | Co zawiera                                                                                                                                                                         | Źródło                                                  |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| `1-stan-projektu/`   | `status.md` — bieżący stan i wersja; `handoff.json` — przekazanie między sesjami (`current_state` z ostatnim commitem, `next_session` — lista zadań, `blocking_issues`, `test_results`); `decisions.md` — decyzje właściciela | `../memory/` obok repozytorium                          |
| `2-zasady-pracy/`    | Zasady, które właściciel ustalił z asystentem (budowanie, testowanie, pobrania, głos, przeglądy interfejsu); `MEMORY.md` to spis treści                                            | `~/.claude/projects/<projekt>/memory/` na komputerze    |
| `3-dziennik-sesji/`  | Notatki z kolejnych dni pracy (`today-RRRR-MM-DD.md`), skrót ostatniego tygodnia (`recent.md`), archiwum                                                                           | `../.remember/` obok repozytorium                       |

## Od czego zacząć

1. `1-stan-projektu/status.md` — co jest zrobione, co otwarte, jak sprawdzać.
2. `1-stan-projektu/handoff.json` → `next_session` (zwykła lista zadań, bez podpól) — co robić dalej;
   `blocking_issues` — co przeszkadza w pracy (dysk, miejsce, maszyna).
3. `2-zasady-pracy/MEMORY.md` — zasady obowiązujące przy każdej zmianie.

Plan i historia zmian są osobno: [`../plan/`](../plan/README.md), [`../../CHANGELOG.md`](../../CHANGELOG.md).

## Zasady

- Plików w podkatalogach nie edytuj tutaj — zmiana zniknie przy następnej kopii. Edytuj źródło i uruchom
  `bash scripts/memory-sync.sh`.
- Żadnych sekretów: skrypt odmawia kopii, gdy znajdzie coś, co wygląda na klucz.
- Projekt jest prowadzony na zmianę na laptopie i komputerze stacjonarnym (dysk E: przechodzi między nimi), a pamięć
  asystenta w `~/.claude/projects/<projekt>/memory/` jest osobna na każdej maszynie. Kopia jest lustrem — na maszynie,
  na której tej pamięci brakuje, najpierw skopiuj zasady z `2-zasady-pracy/` do źródła, dopiero potem uruchom
  `memory-sync.sh`; po kopii sprawdź `git status docs/pamiec-projektu` (usunięcie `D` = coś zginęło u źródła).
- `memory-sync.sh` uruchamiaj w głównej kopii roboczej (`Alfa AI Program/alfa`) — w osobnym drzewie roboczym
  (`git worktree`) katalog nadrzędny nie zawiera `memory/` ani `.remember/`, więc skrypt pominąłby `1-stan-projektu/`
  i `3-dziennik-sesji/`, a mimo to zapisał nową datę kopii.
