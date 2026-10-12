# 21 — Macierz funkcji Delty AI, część 2: wiedza, komputer, modele, twórczość, awatar, multimedia, reszta

Stan na **2026-10-11**. Ciąg dalszy [`20`](20-macierz-delty-czesc-1.md); te same oznaczenia decyzji.

## F. Wiedza, sieć, osoby

| Nr | Funkcja w Delcie | W Alfie dziś | Decyzja | Grupa |
| --- | --- | --- | --- | --- |
| F1 | „Naucz”: kadr z kamery + opis pisany albo dyktowany, tytuł, kategoria (rzecz, przedmiot, miejsce, fakt, notatka), tagi, zdjęcie, przypisanie do osoby | „Zapamiętaj” w pamięci | przenosimy | P5 |
| F2 | „Wiedza”: szukanie, edycja, usuwanie, konsolidacja (co 5 nowych), eksport, import (scal / zastąp), ponowne indeksowanie | Inspektor pamięci, porządkowanie nocne | dostosowujemy: import i eksport wiedzy | P5 |
| F3 | Panel RAG: dokumenty do 20 plików po 25 MB, fragmenty 1100 znaków z zakładką 150, 8 wyników, próg 0,40, wyszukiwanie semantyczne / mieszane / dokładne | wyszukiwanie hybrydowe FTS5 + wektory | dostosowujemy: suwaki progu i liczby fragmentów | P5 |
| F4 | Wyszukiwarka: wyniki + encyklopedia, historia 30 zapytań, popularne, „Odpowiedź AI” | brak | przenosimy | P5 |
| F5 | Wiadomości: odświeżanie przez wyszukiwanie, kategorie, streszczenie, auto-odświeżanie 5–60 min, odczyt głosem | brak | przenosimy | P5 |
| F6 | Ulubione strony: 10 kategorii, własne źródła, wykrywanie RSS | brak | przenosimy | P5 |
| F7 | Osoby: nazwa, znana / tymczasowa, główna, notatki, ostatnio widziana; fakty z kategorią, źródłem i pewnością | SPEC `person-memory` | przenosimy | P6 |
| F8 | Rejestracja twarzy (próg 0,35) i głosu (0,62, margines 0,06) | profil głosu właściciela | przenosimy | P6 |
| F9 | Tożsamość: aktywna osoba 15 s, ponowne rozpoznanie najwyżej co 8 s, grupowanie nieznanych twarzy (wyłączone) | brak | przenosimy | P6 |
| F10 | Historia rozmów osoby (streszczenia) | brak | przenosimy | P6 |

## G. Komputer

| Nr | Funkcja w Delcie | W Alfie dziś | Decyzja | Grupa |
| --- | --- | --- | --- | --- |
| G1 | Głośność systemu: odczyt, ustaw, głośniej, ciszej, wycisz (krok 10) | brak | przenosimy | P7 |
| G2 | Uruchom program, plik albo adres (16 nazw skrótowych; lista zakazanych programów) | brak | przenosimy przez Brokera | P7 |
| G3 | Lista okien, przełączenie okna | UIA w budowie | przenosimy | P7 |
| G4 | Schowek: odczyt i zapis (osobne uprawnienie) | schowek z ochroną | jest | — |
| G5 | Klawisze multimediów | brak | przenosimy | P7 |
| G6 | Mysz: ruch, klik, dwuklik, prawy, przewijanie; klawiatura: pisanie, skróty | `SendInput` w dyktowaniu | przenosimy przez Brokera | P7 |
| G7 | Klik po tekście z ekranu, z próbą bez klikania | brak | przenosimy | P7 |
| G8 | 8 uprawnień (ekran, mysz, klawiatura, pliki, schowek, auto-potwierdzanie plików, skan sieci, procesy) | poziomy L0–L4 i zdolności Brokera | dostosowujemy — bez „auto-potwierdzania” jako flagi modelu | P7 |
| G9 | Panel komputera: nazwa, system, czas pracy, pamięć, dysk; zrzut ekranu | strona Urządzenia, panel Ekran | jest | — |
| G10 | Menedżer procesów: 5 najcięższych, szukanie, sortowanie, zakończenie z potwierdzeniem | brak | przenosimy | P7 |
| G11 | Spis sprzętu: USB, ekrany, dźwięk, dyski, pamięć, płyta, sieć, Wi-Fi, połączenia; skan sieci za zgodą | strona Urządzenia (podstawy) | przenosimy | P7 |
| G12 | Dziennik działań na komputerze | audyt Brokera | jest (pełniejszy) | — |
| G13 | Menedżer plików (tylko katalog wysyłek) | panel Pliki | nie — Alfa ma lepszy | — |

## H. Dostawcy i dobór modelu

| Nr | Funkcja w Delcie | W Alfie dziś | Decyzja | Grupa |
| --- | --- | --- | --- | --- |
| H1 | Dostawcy rozmowy: lokalny, Ollama, OpenAI, Anthropic, Google, OpenRouter + 11 zgodnych z OpenAI (DeepSeek, Qwen, Moonshot, Zhipu, MiniMax, Mistral, xAI, Groq, Cerebras, Together, Fireworks) | katalog dostawców w TOML | jest; uzupełnić listę | P2 |
| H2 | Listy modeli pobierane na żywo, pamięć 24 h | lista modeli konta | jest | — |
| H3 | Stan środków na koncie (tylko OpenRouter) | planowane C2.11a | przenosimy | P2 |
| H4 | Inteligentny wybór: długość, kod, rozumowanie, wiele kroków, obraz; progi oszczędny / zrównoważony / jakość | klasy zadań | przenosimy sygnały trudności | P12 |
| H5 | Pula modeli z rolami (rozmowa, kod, analiza, twórcze, wizja), premia za model już wczytany | kolejki na klasę | dostosowujemy | P2 |
| H6 | Zużycie i koszt na dostawcę z tabeli cen | koszty z kursem NBP, limity | jest (lepsze) | — |

## I. Modele lokalne i karty graficzne

| Nr | Funkcja w Delcie | W Alfie dziś | Decyzja | Grupa |
| --- | --- | --- | --- | --- |
| I1 | Sklep modeli: 4 zakładki, wiersz z parametrami, rozmiarem, oceną dopasowania, pobraniami | stały katalog pakietów | przenosimy | P2 |
| I2 | Filtr rozmiaru (≤3B, 4–9B, 10–15B, 16–35B, >35B), kategorie (9), sortowania | brak | przenosimy | P2 |
| I3 | Ocena: mieści się (plik + 1,5 GB ≤ pamięć karty) / część na procesorze (≤ 1,4 ×) / za duży | ocena pakietów; od 0.0.39 ocena przy każdym modelu rozmowy | przenosimy — **zrobione 0.0.39** (mieści się / częściowo / procesor; bez progu „za duży”) | P2 |
| I4 | HuggingFace: 60 na stronę, „Załaduj więcej”, lista plików repozytorium, kolejka pobierania | brak | przenosimy (pobrania z sumą kontrolną) | P2 |
| I5 | Modele w częściach: komplet w kolejce, znak „Niepełny”, blokada wczytania | brak | przenosimy | P2 |
| I6 | Ostrzeżenia o nieobsługiwanej kwantyzacji | brak | przenosimy | P2 |
| I7 | Archiwum modeli z SHA-256, przywracanie, „Usuń na stałe” | brak | przenosimy (usuwanie na stałe tylko ręką użytkownika) | P2 |
| I8 | Katalog modeli: zmiana folderu, miejsce na dysku, liczniki | ścieżki modeli | przenosimy | P2 |
| I9 | Parametry wczytania: warstwy na karcie, kontekst, wątki | automatyczne | przenosimy | P2 |
| I10 | Parametry silnika: flash attention, typy pamięci KV, mmap, mlock, tryb podziału, wsad, dekodowanie spekulatywne, pamięć podręczna | brak w oknie | przenosimy | P2 |
| I11 | Profile rodzin modeli (Gemma, Qwen, Llama, Mistral, Phi): próbkowanie, myślenie, ograniczenie kontekstu | brak | przenosimy | P2 |
| I12 | Poprawki szablonów: łączenie komunikatów systemowych i kolejnych tur, spłaszczanie narzędzi, system w pierwszej turze, ponowienie po błędzie 400 | filtr zmyślonych wywołań | przenosimy | P2 |
| I13 | Strażnik tur: zakaz znaczników tur, żeby model nie dopisywał wypowiedzi użytkownika | brak | przenosimy | P2 |
| I14 | Dopasowanie do pamięci karty: dobór kontekstu do luzu, `--fit-target`, pomiar faktycznych warstw | automatyczne w `llama-server` | przenosimy | P2 |
| I15 | 5 stopni awaryjnego startu (pełny → układ auto → bez wizji → bez przyspieszeń → minimalny) | zapas CPU | przenosimy | P2 |
| I16 | Model widzący: dobór projektora do rodziny i rozmiaru | czeka na C3.6 | przenosimy | P2 |
| I17 | Wybór karty dyskretnej; interfejs na karcie zintegrowanej | role kart | jest; dochodzi przełącznik | P2 |
| I18 | Warianty silnika CUDA / Vulkan / ROCm: przełącz, zainstaluj, „Przywróć Vulkan” | pakiety silników CUDA / Vulkan / CPU | przenosimy przełączanie; ROCm przez WSL — nie (skrypt) | P2 |
| I19 | Aktualizacja silnika: pobierz → sprawdź → podmień; stara wersja do pierwszego udanego wczytania | aktualizator programu z wycofaniem | przenosimy dla silników | P2 |
| I20 | Wczytanie przy starcie; zwolnienie po bezczynności (300 s, 30–7200) | zwalnianie pamięci okna | przenosimy | P2 |
| I21 | Strojenie do sprzętu: wątki, wsad, kontekst | klasa sprzętu i zalecenie | jest; uzupełnić | P2 |
| I22 | Lista polecanych (ok. 70) i polskie rodziny na żywo | pakiety 1–6 | przenosimy; lista do przeglądu licencji | P2 |

## J. Twórczość

| Nr | Funkcja w Delcie | W Alfie dziś | Decyzja | Grupa |
| --- | --- | --- | --- | --- |
| J1 | Studio obrazu: tekst → obraz, obraz → obraz (siła 0,75) | brak | przenosimy | P8 |
| J2 | Silnik lokalny: SD 1.5, SDXL, FLUX.1, FLUX.2 Klein, Chroma; model rozmowy zwalnia kartę | brak | przenosimy jako proces poboczny | P8 |
| J3 | Parametry: wymiary, kroki 1–200, CFG, ziarno, seria 1–8, 9 próbników, 8 proporcji | brak | przenosimy | P8 |
| J4 | LoRA, ControlNet, IP-Adapter (w Delcie częściowo) | brak | przenosimy LoRA; reszta po sprawdzeniu | P8 |
| J5 | Kreator promptu: 20 stylów, światło, nastrój, kamera, kompozycja, jakość, 5 zestawów negatywnych | brak | przenosimy | P8 |
| J6 | Tłumaczenie promptu na angielski i „ulepsz” | brak | przenosimy | P8 |
| J7 | Profile, szacunek pamięci, seria do 50 promptów, anulowanie | brak | przenosimy | P8 |
| J8 | 7 dostawców w chmurze, kolejka zapasowa, szacunek kosztu, wykrywanie modeli | brak | przenosimy (koszt w limitach Alfy) | P8 |
| J9 | Tryb hybrydowy: wzbogacenie promptu, szkic i dopracowanie, wiele etapów | brak | przenosimy | P8 |
| J10 | Galeria: szukanie, filtr modelu, podgląd z metadanymi, archiwum | brak | przenosimy; dochodzą ulubione i „użyj promptu ponownie” | P8 |
| J11 | Studio opowieści: komiks / książka / artykuł, 1–24 strony, „biblia” stylu i postaci, eksport | brak | przenosimy; dochodzi poprawianie pojedynczej strony | P8 |
| J12 | Menedżer kamer: do 3, okno na kamerę, analiza ciągła, rozpoznanie osoby, nauka z kadru, tryb pokazowy | strona Kamery (USB) | przenosimy brakujące | P8, C3 |
| J13 | Panel Audio: tekst → mowa → pobierz; nagranie → tekst → kopiuj | czytanie na głos, dyktowanie | przenosimy jako jeden panel | P8 |

## K. Awatar

| Nr | Funkcja w Delcie | W Alfie dziś | Decyzja | Grupa |
| --- | --- | --- | --- | --- |
| K1 | 5 trybów: kula, 2D, 2.5D z fizyką, 3D, modele VRM / GLB / FBX | znak agentki z ustami wg głośności | przenosimy | P9 |
| K2 | Rozmiary, włącznik, osobne okno | brak | przenosimy | P9 |
| K3 | Menedżer modeli: import przeciągnięciem, ulubione, 8 dołączonych awatarów | brak | przenosimy; dołączone modele po sprawdzeniu licencji | P9 |
| K4 | Ustawienia kuli, 2D, 2.5D, 3D (oddech, mruganie, wzrok, włosy, wiatr, klatki 15–60) | brak | przenosimy | P9 |
| K5 | Ustawienia modelu 3D (kamera, światło, tło) — w Delcie niepodłączone | brak | przenosimy działające | P9 |
| K6 | 16 emocji z oznaczeń i słów; kolejność: ręczna > intonacja > treść | brak | przenosimy | P9 |
| K7 | Ruch ust: głośność i 12 kształtów z zegarem dźwięku | poziom głośności | przenosimy | P9 |
| K8 | Regulator jakości (15–120 klatek), pauza w tle, ograniczenie ruchu | reguła ograniczenia ruchu | przenosimy | P9 |

## L. Multimedia

| Nr | Funkcja w Delcie | W Alfie dziś | Decyzja | Grupa |
| --- | --- | --- | --- | --- |
| L1 | Odtwarzacz: kolejka, poprzedni / następny, powtarzanie, przewijanie, głośność, wideo | brak | przenosimy | P10 |
| L2 | Most sterowania: polecenia odtwarzania z rdzenia do okna, „co gra” | brak | przenosimy przez zdarzenia Alfy | P10 |
| L3 | Polecenia głosem i tekstem („włącz muzykę”, „głośniej” — odtwarzacz czy system) | brak | przenosimy | P10 |
| L4 | Wyszukiwanie i pobieranie z serwisów wideo | brak | wstrzymane — osobna decyzja właściciela | P10 |
| L5 | Biblioteka, playlisty, historia | brak | nie (decyzja właściciela) | — |

## M. Trening, diagnostyka, przepływy, start

| Nr | Funkcja w Delcie | W Alfie dziś | Decyzja | Grupa |
| --- | --- | --- | --- | --- |
| M1 | Laboratorium: projekty, obrazy z opisami, konfiguracja dla zewnętrznego narzędzia, odczyt wyników | brak | przenosimy; samo trenowanie — kandydat etapu H | P11 |
| M2 | Pulpit testowy: „Testuj wszystko”, „Testuj 2×”, 32 testy w 8 grupach, historia 6 przebiegów, raport | Zdrowie systemu, diagnosta | przenosimy testy i raport | P11 |
| M3 | Diagnostyka okna: testy połączenia, sprzętu i przeglądarki, ocena, podpowiedzi | Zdrowie systemu | dostosowujemy | P11 |
| M4 | Migawki diagnostyczne: zapis, porównanie (do 100, 60 dni) | brak | przenosimy | P11 |
| M5 | Przepływy: kanwa z paletą, siatką, minimapą; tryby Projekt / Testuj / Analiza | brak | przenosimy | P11 |
| M6 | Węzły: wejście tekstu, model, szablon, parser JSON, warunek, pamięć sesji, pamięć długa, RAG, wyjście | brak | przenosimy; 5 węzłów „wkrótce” Delty — do macierzy „wkrótce” Alfy | P11 |
| M7 | Wykonawca: tylko graf bez pętli, 100 węzłów, 500 kroków, 120 s na węzeł, 300 s na przebieg | limity zadań | przenosimy | P11 |
| M8 | „Ustaw jako mózg rozmowy” | brak | przenosimy | P11 |
| M9 | Instalator: środowisko, pakiety, silniki, katalog modeli | instalator programu, pakiety modeli | nie (Python) | — |
| M10 | Bramka gotowości przy starcie z testami i podpowiedziami | ekran ładowania | przenosimy | P3 |

## N. Rzeczy w Delcie zaplanowane, niezrobione — kandydaci

| Nr | Temat | Gdzie w planie Alfy |
| --- | --- | --- |
| N1 | Głos premium z klonowaniem (Chatterbox), pełny dupleks, odszumianie, rozróżnianie mówiących | H.3; odszumianie Alfa ma |
| N2 | Model pomocniczy do dekodowania spekulatywnego, budżet rozumowania, własny szablon rozmowy | P2 — do oceny |
| N3 | Kilka kart w trybie automatycznym | P2 |
| N4 | Awatar: śledzenie twarzy, nowy silnik graficzny, profile Eco / Ultra | H |
| N5 | Linux i własny system | etap G |
| N6 | Agent programista wykonujący naprawdę | P12 |
| N7 | Wiadomości: karty w rozmowie, licznik nowych, usuwanie powtórek | P5 |
| N8 | Obrazy: domalowywanie, powiększanie; ulubione w galerii | P8 |
| N9 | Węzły przepływu: plik, zdarzenie z sieci, przekształcenie, API, poczta | P11, P14 |

## Postęp po wersji 0.0.43 (2026-10-12)

Pozycje macierzy ruszone w nocy 11/12 października: A23 i I3 — zrobione; A34, A35, B11 — częściowo. Rozmowa
i mowa (grupa P1) nie ma wierszy w macierzy — jej stan jest w [`19`](19-specyfikacja-glosu-delty.md)
i w [`17`](17-przeniesienie-z-delty.md). Raport: [`23`](23-raport-nocny-2026-10-12.md).

## Podsumowanie liczbowe

Wierszy w obu częściach (bez listy kandydatów N): 208 — grupują ok. 400 pojedynczych funkcji. Decyzje: przenosimy — 146 ·
dostosowujemy — 20 · jest — 38 · nie — 3 · wstrzymane — 1.
