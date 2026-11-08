---
name: alfa-budowanie-aplikacji
description: Wersję Alfy podnosić i budować dopiero co 10 ukończonych punktów (albo na polecenie) — nie po każdym punkcie
metadata:
  node_type: memory
  type: feedback
  originSessionId: 526ab23b-611a-489a-a8a3-7c1153e69825
  modified: 2026-10-15T14:14:29.868Z
---

2026-10-15 właściciel (Spopielony): „zacznij dalsze etapy i nie buduj wersji co jeden punkt tylko co 10”.
Zastępuje zasadę z 2026-10-11 („co pięć dużych ulepszeń”).

**Why:** budowa powłoki, pełna weryfikacja i wgranie zjadają czas pracy nad funkcjami; po sprzątaniu
dysku (15.10) pierwsza budowa od zera trwa 20–30 min.

**How to apply:** punkt = jedno ukończone zadanie z roadmapy (z testami celowanymi: Rust danego modułu,
vitest, e2e strony). Po każdym punkcie: commit, wpis w `docs/plan/03-roadmapa.md`, licznik w
`memory/status.md` — bez podnoszenia wersji, bez budowy powłoki, bez `e2e-app` i bez wgrywania. Co 10
punktów (albo na polecenie): jedna wersja z wpisem CHANGELOG obejmującym wszystkie punkty, `set-version`,
pełny clippy, `verify-app` (budowa + prawdziwa aplikacja, bramka pokrycia 70 %), wgranie do
`Alfa-uruchomiona`. W raporcie pisać wprost, co nie było jeszcze zbudowane. Patrz też [[alfa-weryfikacja-ui]].
