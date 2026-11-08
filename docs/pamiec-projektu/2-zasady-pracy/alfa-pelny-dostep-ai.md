---
name: alfa-pelny-dostep-ai
description: "Pełny dostęp i tryby zgód jak Claude Code/Codex — od 6.11 etap Z scalony (0.0.85), Alt+M, sędzia przy małym modelu zatwierdzony"
metadata:
  node_type: memory
  type: project
  originSessionId: 526ab23b-611a-489a-a8a3-7c1153e69825
  modified: 2026-11-03T17:43:50.817Z
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

**Decyzja 2026-11-03 (zastępuje menu 4 trybów z 27.10; plan alfa/docs/plan/50):** zgody mają działać na tej samej
zasadzie co Claude Code i Codex na PC — menu 5 trybów: Plan · Ręcznie · Akceptuj edycje (= Codex „Poproś o
zatwierdzenie”) · Automatycznie (decyduje **model-sędzia**, bezpiecznik po 3 odmowach z rzędu) · Pełny dostęp (nigdy
nie pyta); pod listą czerwony przełącznik z poświatą **„Max dostęp”** z wyborem lokalne/chmura/wszystkie (zero pytań);
okno zgody z opcjami Tak / Tak i nie pytaj o <zakres> / Tak i przełącz na Automatycznie / Nie — zrób inaczej.
**Tryb dziecka:** bez PIN-u tylko Plan i Ręcznie; reszta na kłódkę otwieraną wyłącznie **PIN-em rodzica**; dziennik
rodzica pod PIN-em jak ADR 0029 (działania, tematy, alerty; pełna treść po jawnym włączeniu).

**Decyzja 2026-10-31 (zastępuje pkt 2 z 27.10):** na „Pełnej automatyce” (L4) okno zgód **nie pokazuje się pod
żadnym pozorem, aż właściciel obniży poziom** — także wysyłka po obcej treści (TaintedEgress, Trifecta) i obcy/niepewny
głos idą bez pytania („Zrób bez pytania”, wybrane świadomie po opisie ryzyka wycieku przez ukryte polecenia).
Zostają tylko twarde blokady Jądra (odmowa bez okna), STOP Ctrl+Shift+F12 i dziennik. Ten sam dzień: Broker ma się
w UI nazywać **„Sędzia Cieni”** z „?” w kółku (opis, co robi i do czego się odnosi); okno Brokera właściciel ocenia
jako „tragiczne i niedokładne” i irytuje go, że zgłaszał to wiele razy — poprawiać gruntownie, nie punktowo.

**Decyzja 2026-11-06 rano (stan):** etap Z (plan 50) **scalony** do `ccr-af4b63c6-3fyzaj` za zgodą właściciela
(„scalić gałąź tak”) — wersja 0.0.85. Sędzia przy małym modelu lokalnym (sam zatwierdza tylko proste, odwracalne
działania; zapora `guard()` poza modelem) — zatwierdzony. W rozmowie z kłódką „do końca rozmowy” tryby zmienia się
w każdej chwili; Alt+M = następny tryb jak Shift+Tab w Claude Code (nigdy Pełny dostęp). Skrót PIN-u (Argon2id)
zostaje w prywatnym pliku Sędziego, nie w Menedżerze poświadczeń.
