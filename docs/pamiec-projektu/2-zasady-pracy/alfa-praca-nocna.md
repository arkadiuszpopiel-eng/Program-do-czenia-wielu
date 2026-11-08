---
name: alfa-praca-nocna
description: "Alfa — sesja nocna bez właściciela kończy się raportem, zaktualizowanym planem HTML i zbudowaną wersją"
metadata:
  node_type: memory
  type: feedback
  originSessionId: 526ab23b-611a-489a-a8a3-7c1153e69825
  modified: 2026-10-12T01:38:42.920Z
---

Gdy właściciel zleca pracę „do rana” bez swojego udziału (pierwszy raz 2026-10-11, polecenie: „od 22:18 do 5:00
pracuj cały czas… o 5:00 chcę raport z prac i zaktualizowany plan html, z ostatnią wykonaną i zbudowaną przez ciebie
wersją”), na koniec mają być trzy rzeczy: raport z prac, odświeżony plan w HTML i **zbudowana** ostatnia wersja —
także wtedy, gdy nie wypada próg z [[alfa-budowanie-aplikacji]] (właściciel nazwał to wprost regułą na tę noc).

**Why:** właściciel wraca po kilku godzinach i chce od razu zobaczyć wynik w działającym programie oraz przeczytać,
co się zmieniło; nie chce być w nocy pytany o zgody.

**How to apply:** raport pisać jako dokument planu (`alfa/docs/plan/NN-raport-…md` — trafia do HTML), z rozdziałem
„czego nie sprawdziłem”; końcową budowę i testy zacząć co najmniej **3 godziny** przed terminem (2026-10-16: pełne `verify wersja` pod obciążeniem maszyny trwało ok. 2,5 h — sam etap testów Rust 60 min, przeglądarka ponad 40 min; 80 minut nie wystarcza). Przyspieszenie: równolegle z etapami testów zbudować powłokę tym samym poleceniem co `verify-app.sh` (`target/shell`, te same zmienne) — etap aplikacji kończy się wtedy w kilka minut; nie odtwarzać dźwięku
w nocy (próby głosu robić nagraniem zamiast mikrofonu); pilnować dysku — patrz [[alfa-srodowisko-lokalne]].
Moja interpretacja: dotyczy sesji nocnych zleconych wprost, nie zwykłej pracy w dzień.
