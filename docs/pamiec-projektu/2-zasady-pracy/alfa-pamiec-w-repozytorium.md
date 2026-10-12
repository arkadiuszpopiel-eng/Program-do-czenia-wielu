---
name: alfa-pamiec-w-repozytorium
description: "pamięć projektu i wszystko z nią związane ma mieć kopię w katalogu projektu (docs/pamiec-projektu), odświeżaną na końcu sesji"
metadata:
  node_type: memory
  type: feedback
  originSessionId: 526ab23b-611a-489a-a8a3-7c1153e69825
  modified: 2026-10-12T13:50:12.479Z
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
