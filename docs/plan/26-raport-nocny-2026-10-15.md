# Raport z nocy 2026-10-14 / 15 (wersje 0.0.52 – 0.0.54)

Praca od ok. 23:15 do 5:00 na Twoje polecenie: „kontynuuj ulepszenia wraz z testami; decyzje z dokumentu 25 wykonaj
tak, jak uważasz za najlepsze dla programu; o koncie Google przypomnij o 10:00”. Ostatnia w pełni sprawdzona wersja
to **0.0.53**; ostatnia zbudowana i wgrana — **0.0.54** (jedna poprawka, sprawdzenie węższe — niżej). Ten dokument mówi, co doszło, jak to sprawdziłem, czego nie sprawdziłem i co zostaje.

## 1. Najważniejsze w czterech punktach

- **Polecenia powłoki agentek nie zapiszą niczego poza katalogiem roboczym** — nawet jeśli analiza polecenia czegoś nie
  zauważy. Robi to sam Windows (niska integralność procesu), a nie heurystyka Alfy. Git, cargo, node i PowerShell
  działają jak dotąd.
- **Wszystkie cztery decyzje z dokumentu 25 są rozstrzygnięte i wdrożone** (opis w [dokumencie 25](25-do-przegladu-po-audycie.md)).
- **Długie odpowiedzi wyświetlają się prawie trzy razy szybciej**: 100 KB w 7,8 ms (było 22–24 ms, budżet 20 ms).
- **Przypomnienie o 10:00** (test konta Google przez Antigravity) jest ustawione jako zadanie w Claude — przyjdzie
  powiadomieniem z planem testu.

## 2. Co doszło

| Wersja | Co doszło dla użytkownika                                                                                                                                                               |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0.0.52 | Ustawienia → Stan połączeń; sprawdzanie logowania po „Gotowe”; Antigravity (logowanie i most z ostrzeżeniem)                                                                            |
| 0.0.53 | Izolacja poleceń powłoki (integralność Low); Markdown w wątkach; wasmtime 36.0.18 (RUSTSEC-2026-0363); testy prawdziwej aplikacji obejmują 71,8 % poleceń; poprawki po przeglądzie kodu |
| 0.0.54 | Werdykt Krytyczki rozpoznawany mimo literówki lekkiego modelu (poprawna odpowiedź nie idzie już do zbędnej poprawki) |

### Decyzje z dokumentu 25 — co wybrałem i dlaczego

1. **Izolacja poleceń: integralność Low, nie AppContainer.** AppContainer odcina proces od profilu użytkownika — git
   nie przeczyta `~/.gitconfig`, cargo i npm swoich katalogów, więc agentki straciłyby zwykłe narzędzia. Integralność
   Low zostawia odczyt, a blokuje zapis poza katalogiem roboczym (katalog dostaje etykietę Low). Bez izolacji idą:
   polecenia sieciowe, instalacje, polecenia z celami poza katalogiem (one i tak wymagają Twojej zgody), narzędzia
   piszące w profilu (cargo, npm, pip…) i katalogi zbyt szerokie (profil, korzeń dysku, katalogi systemu). Gdy
   izolacji nie da się zastosować (np. dysk exFAT), polecenie działa jak dotąd, a wynik to mówi.
2. **Macierz dowodów** — generowany rejestr `docs/DOWODY.md` z raportów `verify`, od 0.0.51.
3. **Antigravity** — wpis w rejestrze zatwierdzony zgodnie z Twoim wyborem (szara trasa, domyślnie wyłączona).
4. **Budżet Markdown** — dotrzymany bez zmiany budżetu i bez starszej, podatnej biblioteki: czyszczenie HTML w kilku
   wątkach, zabezpieczenie bez zmian, wynik identyczny co do znaku (test).

## 3. Jak to sprawdziłem

- Izolacja na **prawdziwych procesach Windows** (`platform-windows-impl/tests/exec_low.rs`): cmd i PowerShell piszą
  w katalogu roboczym (także nadpisują istniejące pliki), zapis obok kończy się odmową systemu, odczyt plików systemu
  i katalog tymczasowy działają, tryb Medium bez zmian, profil i korzeń dysku nigdy nie dostają etykiety.
- Polityka w narzędziu powłoki (`tools-shell-impl/tests/run.rs`): które polecenia dostają izolację, powrót do Medium,
  wyłącznik `tools.shell.low_integrity`.
- Przegląd kodu w świeżym kontekście (osobna sesja): 2 ważne i kilka drobnych uwag — wszystkie poprawione tej nocy
  (szczegóły w historii zmian 0.0.53, sekcja „Poprawki po przeglądzie”).
- Test prawdziwej aplikacji: 32 testy, pokrycie łącza **168 z 234 poleceń (71,8 %)**; bramka podniesiona do 70 %.
- Pełne `verify wersja` 0.0.53 o 3:19: **ZIELONY**, 18 z 18 etapów — Rust 2753 testy (0 czerwonych), interfejs 365,
  przeglądarka 359, prawdziwa aplikacja 32 (pokrycie łącza 71,8 %), zależności i audyt pakietów czyste. Raport:
  `target/verify/0.0.53/raport-wersja.md`. Wcześniejsze biegi padały na przeciążeniu maszyny (mało pamięci,
  27 GB wolnego na E:) — usunąłem 116,6 GB starych plików kompilacji (do odtworzenia), a testy przeglądarkowe
  dostały limity dopasowane do obciążenia.

- Próba obowiązków na prawdziwym modelu lokalnym (0.0.53, ok. 3:30): rano zielony (329 s), wieczór przekroczył
  10 min. Przyczyna: komputer przeciążony (ok. 2 GB wolnej pamięci; model generował ok. 1,4 słowa/s zamiast ok. 11 —
  te same wybory routera co w zielonej próbie z 0.0.52, rano 58 s / wieczór 28 s). Dziennik pokazał przy tym
  prawdziwy błąd: Krytyczka napisała „WERYFIkcja: OK”, a Alfa wzięła to za odrzucenie i zleciła poprawkę —
  naprawione w 0.0.54: testy modułu agentek zielone, clippy czysty, budowa i test prawdziwej aplikacji 32 z 32
  (pokrycie łącza 71,8 %), wgrane do `Alfa-uruchomiona` o 4:03 i sprawdzony start (`wersja=0.0.54`, bez błędów).
  **Pełne `verify wersja` dla 0.0.54 — nie zdążyłem przed 5:00** (trwa ok. 80 min); 0.0.53 przeszła je w całości.

## 4. Czego nie sprawdziłem

- **Antigravity na prawdziwym `agy` z Twoim kontem Google** — z Tobą o 10:00 (zużyłbym Twoje konto bez pytania).
  Format zdarzeń `stream-json` jest przyjęty z opisu trybu bez okna; parser jest tolerancyjny.
- **Izolacji w codziennej pracy agentek** — testy idą na prawdziwych procesach, ale nie na prawdziwych zadaniach
  agentek w Twoich projektach. Polecenie izolowane ma w wyniku dopisek `[izolacja]` — po nim poznasz, czy coś
  przestało działać.
- **Próby sprzętowe z dźwiękiem** — w nocy bez dźwięku (Twoja zasada).

## 5. Co zostaje

- 10:00 — test konta Google / Antigravity (plan w przypomnieniu).
- Testy w przeglądarce przy 4 przeglądarkach naraz padały na przeciążonej maszynie (2,6 GB wolnej pamięci); bramka
  sama dobiera teraz ich liczbę do wolnej pamięci.
- Etykieta Low zostaje na katalogach roboczych agentek (trwała); inne programy o niskiej integralności (rzadkie,
  np. stare piaskownice) też mogą w nich pisać.
