---
name: alfa-samodzielne-testowanie
description: Właściciel wymaga, żebym sam testował więcej — poziomy verify, testy sprzętowe, instancja testowa obok jego Alfy
metadata:
  type: feedback
---

2026-10-12 właściciel (po raporcie 0.0.45): „zrób tak, żebyś mógł sam więcej testować, bo dużo rzeczy pomijasz —
nie tak miało być; przemyśl to dokładnie, potem dalsze prace”. Dotyczyło list „nie sprawdziłem” w raportach.

**Why:** pomijałem rzeczy, które da się sprawdzić bez niego: ok. 40 testów sprzętowych `#[ignore]` (ten laptop jest
maszyną użytkownika), pełne testy projektu przy wersji, polecenia rdzenia w prawdziwej aplikacji (pokrycie zmierzone
dopiero teraz: 27 % → 38 %), próby funkcji modelowych. Pierwszy bieg testów sprzętowych od razu znalazł błąd
(kwarantanna pobrań a ścieżki 8.3).

**How to apply:** plan i stan w `alfa/docs/plan/24-samodzielne-testowanie.md`. Przed wersją:
`node scripts/verify.mjs wersja`; przed budową dla właściciela: `node scripts/verify.mjs sprzet` (testy sprzętowe
`scripts/verify-hw.mjs`, próby obowiązków `scripts/trial/duty.mjs` z oceną `grounding.mjs`). Test prawdziwej aplikacji
biegnie jako instancja testowa (port 9333, `ALFA_E2E_INSTANCE=1`) obok Alfy właściciela — nie zamykać jego okna;
jego Alfa działa z kopii `Alfa-uruchomiona/` (`scripts/deploy-owner.sh`), nie z `target/shell`. Nigdy `taskkill /IM
alfa-desktop.exe`. „Nie sprawdziłem” wolno pisać tylko z powodu: brak sprzętu (kamera), decyzja właściciela, ocena
jego zmysłami; liczby do raportu brać z `target/verify/<wersja>/raport-*.json`. Nowa funkcja = scenariusz na
prawdziwym rdzeniu (`e2e-app`), pokrycie łącza nie może spadać. Powiązane: [[alfa-weryfikacja-ui]],
[[alfa-budowanie-aplikacji]], [[alfa-praca-nocna]].

**Instancja testowa a właściciel (2026-10-12):** właściciel wziął okno instancji testowej za swoją Alfę, przeszedł w nim kreator i zgłosił „wywalenie przy logowaniu” — a to skrypt testu zamknął okno po limicie czasu. Zanim uznam zgłoszenie awarii za błąd programu, sprawdzam w dziennikach, czy okno nie było instancją testową (`target/verify/app/profil-*`, `target/trial-profile`). Od 0.0.49 instancja ma plakietkę w oknie; mimo to uprzedzam właściciela, gdy uruchamiam test prawdziwej aplikacji w czasie jego pracy. Nie podmieniać `USERPROFILE` instancji testowej — WebView2 nie otwiera wtedy portu CDP.
