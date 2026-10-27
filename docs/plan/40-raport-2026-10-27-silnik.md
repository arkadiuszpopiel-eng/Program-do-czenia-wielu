# Raport 27.10: połączenie z silnikiem NextGO.Gray, wersja 0.0.74 i menu „Tryb pracy”

Polecenie właściciela: dokończyć i sprawdzić bieżącą pracę, a potem przygotować systemy Alfy do pełnej pracy
z silnikiem gier **NextGO.Gray** (`E:\PROGRAMY W CZASIE PRAC\Silnik Do Gier AI NextGO.Gray`). W katalogu
silnika nie wolno nic zmieniać, bo pracuje w nim inny model.

## 1. Silnik gier NextGO.Gray — co działa

Nowy moduł `tools-engine` składa się z kontraktu, atrapy, implementacji i specyfikacji
([SPEC](../modules/tools-engine/SPEC.md)). Najpierw przeczytałam dokumentację silnika: README, AGENTS,
PROGRESS, PLAN_PRODUKCJI, SPEC i `docs/`. Potem połączyłam z nim Alfę. Wersja 0.0.74 zawiera:

- **15 narzędzi agentek:**
  - stan instalacji i dokumentacja;
  - start i zatrzymanie serwera;
  - odczyt sceny, strumieniowania i zaznaczenia;
  - transakcja i cofnięcie;
  - krok symulacji i strumieniowanie terenu;
  - raycast i wysokość terenu;
  - zaznaczenie i podmiana shadera;
  - zrzut okna i podgląd PNG.
- **Każde wywołanie idzie przez Brokera:**
  - `fs.read` i `shell.exec` w zakresie katalogu silnika;
  - `gui.control(engine-server.exe)` dla wywołań serwera;
  - zmiany świata są akcjami zmieniającymi stan.
- **Bezpieczna praca serwera:**
  - serwer słucha tylko na pętli zwrotnej, z losowym portem (nigdy 9001);
  - środowisko jest bez kluczy, bez okna konsoli;
  - kill-switch kończy serwer i podglądy;
  - zrzuty i katalog roboczy procesu są w danych Alfy, nie w katalogu silnika.
- **Bez dublowania zmian:** transakcje mają stały identyfikator z przebiegu i treści, więc ponowienie tego
  samego wywołania nie dubluje zmiany. Wysłane żądanie nigdy nie jest ponawiane.
- **Własny klient WebSocket, bez nowych zależności.** Spełnia RFC 6455 i sprawdza `Sec-WebSocket-Accept`.
  Po 90 s ciszy odnawia połączenie, bo serwer rozłącza po 120 s.

**Jak włączyć:** Ustawienia → Zaawansowane → „Katalog silnika gier NextGO.Gray”. Wpisz pełną ścieżkę do
katalogu silnika. Silnik musi być zbudowany, czyli mieć plik `target\release\engine-server.exe`. Puste pole
wyłącza narzędzia. Narzędzia mają agentki z dostępem do powłoki (grupa `shell`, jak w profilu „pełny dostęp”), a w profilu
„własny” wystarczy też sama grupa `engine`. Narzędzia znajdują się też po słowach „silnik”, „nextgo”, „engine”, „game”, „shader”.

**Sprawdzone:**

- testy kontraktowe na atrapie i na prawdziwym silniku, dwukrotnie (przed podziałem pliku i po nim):
  - scenariusz start → transakcja → odczyt → cofnięcie → zatrzymanie;
  - kill-switch;
  - podgląd GPU i zrzut okna z `--capture-revisions`;
- ścieżka w aplikacji (`app-agents/tests/engine_setting.rs`):
  - ustawienie zapisane jak z Ustawień trafia do narzędzi;
  - na prawdziwym silniku: stan „0.1.0 gotowy, sprawdzenie: passed”, start serwera na losowym porcie i
    odczyt sceny;
  - wyczyszczenie pola wyłącza narzędzia;
  - ten test znalazł zły komunikat („Ustawienia → Silnik gier” zamiast „Zaawansowane”) — poprawiony;
- prawdziwa aplikacja (instancja testowa obok Twojej Alfy, osobny profil):
  - pole w Ustawienia → Zaawansowane zapisuje ścieżkę w rdzeniu;
  - „Przywróć domyślne” ją usuwa;
  - zestaw aplikacji 49/49;
- po testach w katalogu silnika nie powstał żaden plik;
- clippy bez ostrzeżeń, 345 testów zmienionych pakietów, `cargo deny` (bans, sources);
- pełny zestaw przeglądarkowy 559/559, interfejs (vitest) 491.

**Pełne sprawdzenie wersji i niezależny przegląd (27.10 wieczorem):**

- niezależny przegląd modułu silnika w świeżym kontekście: bez ustaleń krytycznych; poprawione 2 poważne
  (połączenie w połowie wiadomości po anulowaniu, podwójna transakcja po ponowieniu w następnym kroku) i 3
  drobne (kill-switch w trakcie startu, ramki sterujące według RFC 6455, osobne katalogi zrzutów z limitem 50);
- `verify wersja`: Rust 3252 zielonych / 0 czerwonych / 41 pominiętych, interfejs 491, aplikacja 49 (pokrycie
  łącza 259 z 281), lint, typy, dokumenty, zależności, audyt — zielone; przeglądarka 577 + 1 czerwony
  (axe target-size kapsuły sterowania ekranem w ciasnej belce) → poprawione (przyciski ≥ 24 px), ponowny pełny
  bieg przeglądarki 578/578;
- nowy przegląd interfejsu `overlap-sweep`: nachodzenie elementów i tekst ucięty w pionie w 3 stylach × 2
  motywach, okno 720–1920 px, wszystkie strony Ustawień, panele i trzy profile modelu — 18/18.

**Poza zakresem v0:** zapis i eksport świata oraz zrzut bez okna. Silnik nie ma jeszcze tych metod (to jego
etap 6). Most MCP silnika (`engine-mcp-bridge.exe`) działa niezależnie i można go dodać w Ustawienia →
Serwery MCP.

## 2. Poprawki w wersji 0.0.74

| Nr  | Waga    | Problem                                                                                                      | Poprawka                                                   |
| --- | ------- | ------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------- |
| 1   | poważna | Przycisk „Zatrzymaj” sterowania ekranem w oknie 721–1400 px był pusty: bez nazwy i za mały (axe, 268 błędów) | ikona i nazwa dla czytników                                |
| 2   | poważna | Kapsuła „Delta steruje ekranem” w oknie 1401–1700 px wchodziła pod obsadę agentek i zasłaniała przyciski     | do 1700 px ikona i kwadrat; stan „Sterujesz Ty” z napisem  |
| 3   | lint    | `app-api`, `app-voice` ponad limit długości                                                                  | testy przeniesione do `tests/`, `apply_values` w app-voice |

## 3. Menu „Tryb pracy” — gałąź `do-przegladu-pelny-dostep` (bez scalenia)

Zgodnie z Twoimi wyborami w menu w pasku tytułu są cztery tryby:

- Tylko podgląd;
- Pytaj o wszystko;
- Automatycznie;
- Pełna automatyka.

Tryb wybierasz klawiszami 1–4 albo myszą. Podniesienie potwierdzasz raz w oknie Brokera. Broker zapamiętuje
poziom globalny po restarcie, a tryb obowiązuje we wszystkich rozmowach. Obniżenie działa od razu.

Kod jest gotowy i sprawdzony:

- testy Brokera, klasyfikatora i głosu: 240/0;
- interfejs: 495;
- nowy test menu w 3 stylach i 2 motywach z axe;
- pełny zestaw przeglądarkowy: 564 zaliczone, 1 błąd, poprawiony i sprawdzony ponownie (62/62 testów
  belki).

Zmiana dotyczy Brokera i polityk ryzyka, więc **czeka na Twój przegląd**: commit `a24f7680` na gałęzi.

**Do Twojej decyzji:**

1. **Obcy głos i niepewne rozpoznanie mowy nadal pytają na L4.** Twój wybór brzmiał: na L4 pyta tylko
   „wysyłka po obcej treści”. Przy pełnym zdjęciu reguł głosowych L4 przepuszczałby bez pytania:
   - polecenie wypowiedziane cudzym głosem (np. z telewizora);
   - polecenie rozpoznane z pewnością 55 %, w tym trwałe usunięcie plików.

   To nie jest Twoje polecenie, więc zostawiłam te dwie reguły na każdym poziomie, w tej samej kategorii co
   „obca treść”. Na L4 bez pytania przechodzą operacje administratora i usuwanie Twoim zweryfikowanym,
   pewnie rozpoznanym głosem. Jeśli chcesz zdjąć także te dwie reguły, to jedna linia w `rules.rs` i tabela
   testów.

2. **Nagranie Twojego głosu.** Weryfikacja mówcy nie odróżnia nagrania od żywego głosu (anti-spoofing jest
   poza v1). Na L4 nagranie Twojego głosu może więc usunąć pliki bez pytania. Ryzyko jest opisane w SPEC
   `voice-speaker`.
3. **Sesja z własnym, wcześniej obniżonym poziomem** zostaje przy nim po zmianie trybu globalnego. Menu mówi
   „dotyczy wszystkich rozmów”. Do ustalenia: czy wybór w menu ma też kasować poziomy pojedynczych rozmów.
4. **Okno 721–900 px:** belka nie mieści kapsuły stanu (tak było i wcześniej), więc menu trybu jest tam
   niedostępne. Zostaje Ustawienia → Uprawnienia.
