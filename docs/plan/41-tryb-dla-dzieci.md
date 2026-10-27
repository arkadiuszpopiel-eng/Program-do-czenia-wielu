# 41 — Alfa dla dzieci: nauka, zabawa i podgląd rodzica (etap K)

Stan na **2026-10-28**, wersja 0.0.75. Zlecenie właściciela z 27.10: „Alfa ma służyć też młodszemu pokoleniu
(dzieci, edukacja, zabawa), więc musi być multimedialna i dopracowana, a w przyszłości z podglądem z telefonu,
co dzieci robią na swoim koncie”. Ten dokument to plan; kodu etapu K jeszcze nie ma. Etap opiera się na kontach
i roli **Dziecko** z [`18`](18-wielu-uzytkownikow.md) (etap U) i na dostępie zdalnym z etapów D–F.

## Zasady (nie do negocjacji)

- **Rodzic decyduje.** Konto dziecka zakłada i zamyka właściciel albo Dorosły z prawem opieki; dziecko nie zmienia
  swoich uprawnień, limitów ani filtrów.
- **Bez działań w systemie.** Dziecko nie uruchamia poleceń, nie steruje myszą ani klawiaturą, nie instaluje i nie
  wysyła niczego na zewnątrz. Broker odrzuca takie prośby z konta dziecka bez pytania (twarda blokada roli).
- **Bez chmury domyślnie.** Rozmowa idzie przez modele lokalne; chmura tylko po zgodzie rodzica i tylko u dostawców
  z umową przetwarzania danych dzieci (RODO art. 8, [`10`](10-zgodnosc-prawna.md)).
- **Podgląd, nie inwigilacja.** Rodzic widzi czas, tematy, wyniki nauki i alerty; pełną treść rozmów tylko wtedy,
  gdy ustawi to jawnie, a dziecko widzi znak „rodzic może czytać”.
- **Prawda o AI.** Agentki mówią dziecku wprost, że są programem; nie udają przyjaciółki-człowieka, nie proszą
  o dane osobowe, nie zachęcają do sekretów przed rodzicem.

## Co dostaje dziecko

| Element         | Opis                                                                                                                                                                    |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Wygląd „Junior” | duże przyciski i litery, jasne kolory agentek, pełne animacje awatarów (mówiące twarze, litery z ust — 0.0.75), dźwięki potwierdzeń (wyłączane), mniej opcji na ekranie |
| Rozmowa głosem  | domyślnie mówione odpowiedzi, wolniejsze tempo, wyróżnianie czytanego zdania (pomaga w nauce czytania)                                                                  |
| Nauka           | ścieżki: czytanie, matematyka, języki, przyroda; ćwiczenia generowane do poziomu dziecka; poprawianie bez oceniania „źle”; podsumowanie dla rodzica                     |
| Zabawa          | zagadki, opowieści pisane razem z agentką, rysunki opisywane słowami, quizy z nagrodami w postaci odznak (bez prawdziwych pieniędzy i bez reklam)                       |
| Twórczość       | proste gry i animacje budowane z agentką (silnik NextGO.Gray w trybie piaskownicy — bez dostępu do plików poza katalogiem dziecka)                                      |
| Czas            | limit dzienny i godziny (np. bez Alfy po 20:00); łagodne przypomnienie przed końcem                                                                                     |
| Pomoc           | przycisk „Zawołaj rodzica” — powiadomienie na telefon rodzica                                                                                                           |

## Co dostaje rodzic

| Element                        | Opis                                                                                                                                                                                   |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Strona „Dzieci” w Ustawieniach | konta dzieci, wiek, poziom filtra, limity czasu i kosztów, zgoda na chmurę, kto może czytać rozmowy                                                                                    |
| Raport dzienny i tygodniowy    | czas, ulubione tematy, postępy w nauce, odznaki; w telefonie i na komputerze                                                                                                           |
| Alerty                         | treści niepokojące (przemoc, samookaleczenie, prośby o spotkanie, dane osobowe) — od razu powiadomienie z fragmentem; bez fałszywego alarmu przy zwykłej nauce o historii czy biologii |
| Podgląd z telefonu             | aplikacja na Androida (etap F) i panel WWW (etap E): lista aktywności dziecka na żywo, przerwanie sesji, wydłużenie czasu, odpowiedź na „Zawołaj rodzica”                              |
| Zgody                          | prośby dziecka o coś spoza jego roli trafiają do rodzica jako zgoda w telefonie                                                                                                        |

## Jak to wygląda w budowie

- **Rola Dziecko w Jądrze** (U.6): lista dozwolonych poleceń i zdolności; Broker zna rolę konta i odrzuca resztę;
  testy ataków w U.9 obejmują próbę podniesienia roli z konta dziecka.
- **Filtr treści**: lokalny klasyfikator na wejściu i wyjściu (model ONNX), progi według wieku; zablokowana
  odpowiedź zostaje zastąpiona spokojnym wyjaśnieniem i (przy alertach) powiadomieniem rodzica.
- **Profil „Junior”**: osobny styl wyglądu (`ui.style = junior`) zbudowany na tych samych tokenach co pozostałe;
  każdy stan sprawdzany jak dziś: wszystkie motywy, axe, nachodzenie, szerokości.
- **Podgląd zdalny**: zdarzenia aktywności dziecka (bez treści, chyba że rodzic zezwolił) trafiają przez
  `remote-gateway` (D.1) do sparowanego telefonu rodzica; szyfrowanie końcówka–końcówka jak cały dostęp zdalny.
- **Dane**: pamięć i rozmowy dziecka w jego katalogu konta (U.2); eksport i usunięcie na żądanie rodzica.

## Zadania (roadmapa, etap K)

| ID  | Zadanie                                                                                              | Zależy od               |
| --- | ---------------------------------------------------------------------------------------------------- | ----------------------- |
| K.0 | Decyzje właściciela: przedziały wieku, kto czyta rozmowy, zgoda na chmurę, ADR „Konta dzieci”        | U.0                     |
| K.1 | Rola Dziecko w Jądrze i Brokerze, twarde blokady, testy ataków                                       | U.1, U.4, U.6           |
| K.2 | Filtr treści lokalny (wejście i wyjście), progi według wieku, zestaw testowy zamrożony w `evals/`    | K.1                     |
| K.3 | Styl „Junior”: wygląd, głos, wyróżnianie czytanego zdania, dźwięki                                   | K.1                     |
| K.4 | Nauka: ścieżki, ćwiczenia do poziomu, odznaki, raport dla rodzica                                    | K.2, K.3                |
| K.5 | Zabawa i twórczość: zagadki, opowieści, piaskownica silnika gier                                     | K.2, `tools-engine`     |
| K.6 | Strona „Dzieci” w Ustawieniach: limity czasu, godziny, koszty, chmura, czytanie rozmów               | K.1                     |
| K.7 | Podgląd z telefonu i panelu WWW: aktywność na żywo, alerty, „Zawołaj rodzica”, zgody                 | K.6, D.2, E.3, F.2, U.8 |
| K.8 | Zgodność: RODO (zgoda rodzica, minimalizacja danych), przegląd prawny, przegląd w świeżym kontekście | K.1–K.7                 |

**Wyjście z etapu K:** dziecko rozmawia i uczy się głosem w stylu „Junior” bez dostępu do systemu i chmury;
rodzic w telefonie widzi raport dnia, dostaje alert w ciągu minuty i może przerwać sesję jednym dotknięciem.

## Otwarte pytania do właściciela

1. Jakie przedziały wieku? (Propozycja: 6–9, 10–12, 13–15; poniżej 6 lat tylko z rodzicem przy komputerze.)
2. Czy rodzic domyślnie czyta pełne rozmowy, czy tylko tematy i alerty? (Propozycja: tematy i alerty.)
3. Czy dzieci mogą korzystać z chmury, jeśli rodzic zgodzi się raz dla konta? (Propozycja: tak, z listą dostawców.)
4. Czy odznaki mają być wspólne dla rodzeństwa (ranking), czy tylko osobiste? (Propozycja: tylko osobiste.)
