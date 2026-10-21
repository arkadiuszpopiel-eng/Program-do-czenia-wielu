---
name: alfa-pamiec-w-repozytorium
description: "pamięć projektu i wszystko z nią związane ma mieć kopię w katalogu projektu (docs/pamiec-projektu), odświeżaną na końcu sesji"
metadata:
  node_type: memory
  type: feedback
  originSessionId: 526ab23b-611a-489a-a8a3-7c1153e69825
  modified: 2026-10-21T13:20:00.000Z
---

2026-10-12 właściciel (Spopielony): „Pamięć projektu i wszystkiego z nim związane mają być też zapisywane w katalogu
projektu, dobrze oznaczone!”.

**Why:** pamięć leżała poza repozytorium (`../memory/`, `~/.claude/projects/…/memory/`, `../.remember/`) — przy
przeniesieniu albo kopii projektu zostawała na starym komputerze.

**How to apply:** na końcu każdej sesji, po aktualizacji `memory/status.md` i `handoff.json`, uruchomić w repozytorium
`bash scripts/memory-sync.sh` (kopiuje do `alfa/docs/pamiec-projektu/`: `1-stan-projektu`, `2-zasady-pracy`,
`3-dziennik-sesji`, z `README.md` i datą kopii) i zacommitować. Źródła zostają na miejscu — w repozytorium jest
lustro, więc nie edytować tam plików ręcznie. Nowy rodzaj notatek o projekcie = nowy opisany podkatalog i wpis
w `README.md`. Żadnych sekretów w pamięci (skrypt odmawia kopii, gdy znajdzie klucz). Patrz [[alfa-zasady-pracy]].
Nowy komputer (2026-10-18, [[maszyna-stacjonarna-2026-10-17]]): pamięć asystenta w `~/.claude` była pusta, a lustro
usunęło z repo 10 plików zasad — przed `memory-sync.sh` na nowej maszynie skopiować zasady z
`docs/pamiec-projektu/2-zasady-pracy` do `~/.claude/projects/…/memory/` i połączyć spis `MEMORY.md`; po synchronizacji
sprawdzić `git status docs/pamiec-projektu` (usunięcia `D` = coś zginęło u źródła).
2026-10-21 ten sam błąd w drugą stronę: pamięć asystenta na PC była STARSZA niż kopia z laptopa, a `memory-sync.sh`
nadpisał nią repozytorium (zasady dwóch maszyn, -j3 na laptopie, spis bez „Pełny dostęp”) — wyłapał to dopiero przegląd.
Po synchronizacji czytać cały wynik (OSTRZEŻENIE, STARSZE ZASADY, WYNIK) i `git diff docs/pamiec-projektu/2-zasady-pracy`
przed commitem: usunięte akapity albo pozycje spisu = stara pamięć u źródła, nie zmiana. Od 21.10 skrypt wstrzymuje
zasadę równą wcześniejszej zatwierdzonej wersji (kod 3), ale spisu z jedną nową pozycją sam nie rozpozna.
