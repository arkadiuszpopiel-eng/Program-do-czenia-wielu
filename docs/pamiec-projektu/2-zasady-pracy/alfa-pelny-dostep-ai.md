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

**Decyzje 2026-10-27 (menu trybów jak w Claude Code, po polsku):** właściciela drażni ciągłe potwierdzanie L4
(dziś: Broker trzyma poziom w pamięci, po każdym starcie wraca L3; każde podniesienie = karta w oknie Brokera).
Wybrał: (1) „Pełna automatyka” potwierdzana **raz** w oknie zgód Brokera i pamiętana po restarcie Alfy/komputera,
aż sam obniży; (2) na „Pełnej automatyce” pyta **tylko** o wysyłkę po kontakcie z obcą treścią (TaintedEgress;
pokrewna Trifecta też zostaje) — operacje administratora i niepewne polecenia głosowe już bez pytań; blokada Jądra
zostaje; (3) tryb **globalny** dla wszystkich rozmów; (4) menu 4 trybów: „Tylko podgląd” (L0), „Pytaj o wszystko”
(L1), „Automatycznie” (L3, domyślny), „Pełna automatyka” (L4). Bez trybów „Akceptuj edycje” i „Najpierw plan”.
Kod Brokera/polityk: osobna gałąź do przeglądu właściciela.
