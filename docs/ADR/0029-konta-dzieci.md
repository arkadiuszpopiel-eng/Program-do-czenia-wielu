# ADR 0029 — Konta dzieci: wiek, podgląd rodzica, chmura, odznaki

| Pole | Wartość |
|---|---|
| Status | **Przyjęty** (decyzje właściciela 2026-10-29: „Wykonaj!” — przyjęte propozycje z planu 41); wdrożenie czeka na etap U |
| Data | 2026-10-29 |
| Powiązane | ADR 0027 (konta, logowanie i role — **Proponowany**, U.0), `docs/plan/41-tryb-dla-dzieci.md` (etap K), `docs/plan/18-wielu-uzytkownikow.md`, `docs/plan/10-zgodnosc-prawna.md`, `docs/plan/43-czytane-zdanie.md` |
| Zmienia | nic w kodzie — ustala zasady etapu K; rola **Dziecko** w Jądrze i Brokerze (K.1) powstaje dopiero po U.1, U.4, U.6 i wymaga przeglądu człowieka |

## Kontekst

Właściciel 27.10: Alfa ma służyć też dzieciom (nauka, zabawa), z podglądem rodzica z telefonu. Plan 41 opisał
etap K i zostawił 4 pytania (K.0). Właściciel 29.10 przyjął propozycje planu bez zmian. Etap K opiera się na
kontach i roli Dziecko z ADR 0027, który czeka na podpis (U.0) — dlatego ten ADR ustala zasady, a nie kod.

## Decyzja

1. **Przedziały wieku: 6–9, 10–12, 13–15 lat.** Każdy przedział ma własne progi filtra treści (K.2), domyślne
   limity czasu i zestaw ścieżek nauki (K.4). Dziecko poniżej 6 lat korzysta z Alfy **tylko z rodzicem przy
   komputerze** — konto dziecka nie przyjmuje wieku poniżej 6 lat; rodzic może uruchomić sesję wspólną na swoim
   koncie. Od 16 lat — konto Dorosłego z ograniczeniami według uznania właściciela (poza etapem K).
2. **Podgląd rodzica domyślnie: tematy i alerty, bez pełnej treści rozmów.** Rodzic widzi czas, tematy, wyniki
   nauki, odznaki i alerty (z fragmentem, którego alert dotyczy). Pełną treść włącza jawnie dla konkretnego
   dziecka na stronie „Dzieci” (K.6); wtedy dziecko widzi stały znak „rodzic może czytać rozmowy”. Zmiana trafia
   do dziennika audytu.
3. **Chmura: jedna zgoda rodzica na konto dziecka, z listą dostawców.** Domyślnie tylko modele lokalne. Rodzic
   może raz zgodzić się na chmurę dla danego konta, wybierając dostawców z listy dopuszczonych — tylko takich,
   którzy mają umowę przetwarzania danych dzieci (RODO art. 8, plan 10). Zgoda jest odwoływalna w każdej chwili,
   ma datę i jest w dzienniku; po odwołaniu rozmowy natychmiast wracają do modeli lokalnych. Przy dziecku
   rozmowy w chmurze nie zapisują pamięci długoterminowej u dostawcy (tryb bez retencji, gdy dostawca go ma).
4. **Odznaki tylko osobiste.** Bez rankingów i porównań między rodzeństwem; bez nagród pieniężnych i reklam.
   Rodzic widzi odznaki każdego dziecka osobno.

## Konsekwencje

- **K.1–K.2 i K.6–K.7 czekają na etap U** (U.0 → U.1, U.2, U.4, U.6, U.8). Rola Dziecko i twarde blokady w
  Brokerze to obszar Jądra/Brokera — tylko z przeglądem człowieka (AGENTS.md).
- **K.3 zrobione 29.10** (interfejs, bez kont; do następnej wersji po 0.0.76): wyróżnianie czytanego zdania
  (plan 43, faza 1 — „Przeczytaj na głos”, faza 2 — rozmowa głosowa), styl „Junior”, propozycja wolniejszej mowy,
  dźwięków i „Uproszczonego ekranu” (`ui.simple`). To zwykłe ustawienia wyglądu i głosu — **wygoda, nie
  blokada**; po etapie U będą domyślne i wymuszone dla roli Dziecko (K.1). Uproszczony ekran nigdy nie chowa
  wskaźników prywatności i bezpieczeństwa (mikrofon, kamera, sterowanie, tryb pracy, Broker).
- **Strona „Dzieci” (K.6)** ma pola: wiek (przedział z punktu 1), poziom filtra, limit dzienny i godziny, limit
  kosztów, zgoda na chmurę z listą dostawców (punkt 3), czytanie pełnych rozmów (punkt 2).
- **Zgodność (K.8)**: przegląd prawny RODO przed udostępnieniem kont dzieci komukolwiek poza właścicielem.

## Odrzucone

- Rodzic domyślnie czyta wszystko — narusza zasadę „podgląd, nie inwigilacja” z planu 41 i minimalizację danych.
- Chmura po zgodzie na każdą rozmowę osobno — męczące dla rodzica, w praktyce prowadzi do zgód „na ślepo”.
- Wspólny ranking rodzeństwa — rywalizacja zamiast nauki; odznaki mają motywować, nie porównywać.
