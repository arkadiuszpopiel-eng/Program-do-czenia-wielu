---
name: alfa-pelny-dostep-ai
description: "Decyzja właściciela 2026-10-20 — poziom „Pełny dostęp” AI do komputera bez pytań o zgodę; zostają wyłącznik, dziennik i ochrona przed poleceniami z obcych treści"
metadata:
  node_type: memory
  type: project
  originSessionId: 526ab23b-611a-489a-a8a3-7c1153e69825
  modified: 2026-10-20T12:46:49.605Z
---

2026-10-20 właściciel zażądał „pełnego dostępu AI do komputera, AI nie może być ograniczone”. Po wyjaśnieniu
wybrał wariant: **nowy poziom autonomii „Pełny dostęp” w Ustawieniach, włączany przez właściciela** — agentki
działają bez pytania o zgodę (pliki, programy, mysz/klawiatura, system). **Zostają:** wyłącznik awaryjny
(Ctrl+Shift+F12), dziennik działań (audyt) i blokada poleceń z obcych treści (prompt injection z e-maili/stron).
Odrzucony wariant „zero ograniczeń”.

**Why:** właściciel chce, by AI wykonywało pracę bez ciągłych zgód; wyłącznik i audyt nie ograniczają AI,
a ochrona przed wstrzyknięciem chroni przed obcymi sterującymi komputerem przez Alfę.

**How to apply:** realizacja po wydaniu 0.0.65 (seria 7). Zmiany w `safety-broker`/politykach wymagają przeglądu
człowieka (AGENTS.md) — przygotować na osobnej gałęzi `do-przegladu-pelny-dostep` z opisem, nie scalać bez
akceptacji właściciela; każdy krok w kodzie zabezpieczeń potwierdzać z właścicielem w bieżącej sesji. Powiązane: [[alfa-zasady-pracy]], [[alfa-samodzielne-testowanie]].
