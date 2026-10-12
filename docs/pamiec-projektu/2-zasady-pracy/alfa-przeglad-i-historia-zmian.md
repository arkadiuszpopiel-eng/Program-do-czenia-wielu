---
name: alfa-przeglad-i-historia-zmian
description: "Standard przeglądu aplikacji Alfa (każde okno, panel, skrót), wagi w dzienniku zmian i spis funkcji „wkrótce”"
metadata:
  node_type: memory
  type: feedback
  originSessionId: 526ab23b-611a-489a-a8a3-7c1153e69825
  modified: 2026-10-11T13:18:59.490Z
---

Właściciel (2026-10-11, polecenie `/goal`): przy przeglądzie aplikacji sprawdzać najmniejsze szczegóły — każde
okno, każdy rozwijany panel, każde okno i funkcję wywoływaną skrótem i myszką; „wszystko musi być perfekcyjne
i wysokiej klasy”. Zauważył w programie dużo funkcji z napisem „wkrótce” i oczekuje, że każda jest zapisana
w planie. Poprosił o trwałą historię zmian w Aktualizacjach z podziałem na krytyczne (czerwone), poważne
(pomarańczowe) i normalne (zielone).

**Why:** okno „Co nowego” znika po jednym pokazaniu i nie zostawia historii; funkcje „wkrótce” istniały tylko
w programie, bez zadań w roadmapie.

**How to apply:**

- Każda nowa pozycja w `alfa/CHANGELOG.md` dostaje wagę: `- [krytyczne] …` (bezpieczeństwo, prywatność, utrata
  danych, funkcja nie działała wcale), `- [poważne] …` (istotny błąd albo zabezpieczenie), bez znacznika — zwykła.
  Program pokazuje to w Ustawienia → Aktualizacje → Historia zmian (`app-updates/src/history.rs`,
  `ChangeHistory.svelte`).
- Strona Ustawień z `wave` > 1 („wkrótce”) musi mieć wiersz w `alfa/docs/plan/14-funkcje-wkrotce.md` i zadanie
  w roadmapie (grupa C4) — pilnuje test `app-core/tests/soon_inventory.rs`.
- Przegląd UI: oprócz reguły z [[alfa-weryfikacja-ui]] (stany otwarte, 3 style × 2 motywy, prawdziwa aplikacja)
  przechodzić także skróty klawiszowe i menu pod prawym przyciskiem; wynik przeglądu zapisywać w
  `alfa/docs/reviews/`. (Moja interpretacja zakresu: pełny przegląd przy każdym etapie, nie przy każdej drobnej
  zmianie — do potwierdzenia, jeśli właściciel oczekuje inaczej.)
