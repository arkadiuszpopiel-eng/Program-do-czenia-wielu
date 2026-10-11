# 20 — Macierz funkcji Delty AI, część 1: powłoka, ustawienia, rozmowa, agentki, narzędzia

Stan na **2026-10-11**. Każda funkcja Delty 0.8.35 znaleziona w kodzie ma tu wiersz z decyzją. Część 2 (wiedza,
komputer, modele, twórczość, awatar, multimedia, diagnostyka, przepływy): [`21`](21-macierz-delty-czesc-2.md).
Porównana ze zrzutami menu Delty od właściciela (2026-10-11): wszystkie doki mają wiersze; dopisane szczegóły
to A15a, A22a, A36, B26, B27. Zasada: jeśli Alfa ma coś lepszego, zostaje Alfa.
Głos: [`19`](19-specyfikacja-glosu-delty.md).

Decyzje: **przenosimy** · **dostosowujemy** (inaczej, bo Alfa ma własny mechanizm) · **jest** (Alfa już to ma) ·
**nie** (z powodem). Kolumna „Grupa” to zadanie etapu P w [roadmapie](03-roadmapa.md).

## A. Nawigacja i powłoka

| Nr | Funkcja w Delcie | W Alfie dziś | Decyzja | Grupa |
| --- | --- | --- | --- | --- |
| A1 | 11 obszarów roboczych, 50 widoków, adres w oknie (`#obszar/widok`) | rozmowa + Ustawienia (32 strony) + 7 paneli | dostosowujemy: nowe widoki dochodzą do układu Alfy | P3 |
| A2 | Widok ładowany na żądanie, karta „Otwieram widok”, „Ponów ładowanie” po błędzie | ładowanie leniwe paneli | jest; dopisać kartę błędu z ponowieniem | P3 |
| A3 | Belka: znak → rozmowa; kula stanu głosu (klik otwiera przepływy) | belka z obsadą agentek i paskiem LED | dostosowujemy: kula stanu obok obsady | P1 |
| A4 | Przypięcie kuli do osobnego okna zawsze na wierzchu | pigułka głosowa | przenosimy jako wariant pigułki | P1 |
| A5 | „Odczep aktywny widok do osobnego okna” (każdy widok; 1180×760, rozmowa 1320×820) | brak | przenosimy | P3 |
| A6 | Przycisk MENU: klik = Rdzeń poleceń, Shift+klik = Macierz narzędzi | paleta poleceń `Ctrl+K` | przenosimy obok palety | P3 |
| A7 | Stan w belce „LOCAL · CPU % · READY” (w Delcie wpisany na sztywno) | prawdziwy stan: profil, autonomia, koszt | jest (lepsze); nie przenosimy atrapy | — |
| A8 | Zamknięcie okna zatrzymuje zaplecze | zamknięcie chowa do zasobnika (ustawienie) | jest | — |
| A9 | Doki chowane do najechania (zamknięcie po 280 ms, przejście 640 ms), jeden naraz | brak | przenosimy | P3 |
| A10 | Dok „Chat”: Chat, Arena, Focus, Rozmowy, Szablony, „Historia rozmów” | lista sesji | przenosimy jako grupy widoków | P3 |
| A11 | Dok „Delta”: przepływy, persona, wiedza, kamera, głos, awatar, osoby, „Naucz” | brak | przenosimy (nazwa: „Alfa”) | P3 |
| A12 | Dok „Narzędzia”: studio obrazu, galeria, media, modele, sklepy, agenci, sieć, komputer, trening | brak | przenosimy | P3 |
| A13 | Dok „Ustawienia”: ustawienia, instrukcja, co nowego, moduły, komendy, skróty, diagnostyka, pulpity | strona Ustawień | dostosowujemy: skrót do stron Ustawień | P3 |
| A14 | Przypinanie doku (tylko na czas sesji) | brak | przenosimy, z zapamiętaniem | P3 |
| A15 | Dok „Funkcje”: 12 przełączników (komputer, mysz i klawiatura, uprawnienia, zrzuty, schowek, Internet, pliki, kod, kamera, multimedia, obrazy) | poziomy autonomii, strona Uprawnienia | przenosimy jako szybkie przełączniki zdolności | P3, P7 |
| A15a | Dok „Funkcje”: zdanie stanu na górze („może klikać i pisać na tym komputerze”) i dwa kolory włączenia — zielony dla zwykłych zdolności, pomarańczowy dla ryzykownych (system, mysz i klawiatura, kod) | objaśnienie stanu w belce | przenosimy | P3 |
| A16 | „Odetnij Deltę od komputera” — jednym kliknięciem wyłącza mysz, klawiaturę, system i kod | STOP WSZYSTKIEGO `Ctrl+Shift+F12` | przenosimy jako łagodniejszy stopień obok STOP | P3 |
| A17 | Dok „Skróty”: własne ulubione widoki z listy wszystkich | brak | przenosimy | P3 |
| A18 | Dok „Okna”: lista odczepionych okien (do 12), Pokaż / Przywróć / Wierzch / Reset / Zamknij | brak | przenosimy | P3 |
| A19 | Układy okna: Laptop (980×700), 2 monitory, Studio 21:9, Live rozmowa, Dev / Coding | brak | przenosimy | P3 |
| A20 | Przenoszenie okna na monitor M1…Mn | brak | przenosimy | P3 |
| A21 | Profile pracy (kilka okien naraz): Centrum pracy, Live rozmowa, Dev / Coding, Media lab, Modele i sklepy | brak | przenosimy | P3 |
| A22 | „Profile układu” zamkniętych okien | brak | przenosimy | P3 |
| A22a | Okna: liczba okien przy profilu pracy; przy zapisanym układzie nazwa widoku, rozmiar i położenie (np. 1196×769 @ 370,136); akcja „W głównym” | brak | przenosimy | P3 |
| A23 | Wstecz / Dalej przy znaku, 50 pozycji, podpowiedź z nazwą celu | brak | przenosimy | P3 |
| A24 | Menu kołowe: F1, F2, środkowy przycisk myszy; 8 pól Rdzenia, 8 grup Macierzy z podmenu; strzałki, Enter, Esc | brak | przenosimy | P3 |
| A25 | Dostrajanie menu kołowego: skala 86–108 %, promień etykiet, rozmiar rdzenia, odstęp, grubość, poświata; 4 przełączniki; import / eksport | brak | przenosimy | P3 |
| A26 | Pulpit kafelkowy: do 12 kafli, domyślnie 8, skale Mały / Średni / Duży (dotykowy) | brak | przenosimy | P3 |
| A27 | Edycja pulpitu: przeciąganie, rozmiar kafla, odczepianie, usuwanie; siatka 1–12 × 1–4; 8 zapisanych układów | brak | przenosimy | P3 |
| A28 | „Odczep pulpit” do osobnego okna | brak | przenosimy | P3 |
| A29 | Szuflada rozmowy: nowa rozmowa, ostatnie rozmowy, pliki sesji, liczniki kontekstu | lista sesji, panel Pliki | jest | — |
| A30 | Pole pisania dostępne na dole wybranych widoków | tylko w rozmowie | przenosimy | P4 |
| A31 | Stały pasek odtwarzacza i pasek pobierania | brak | przenosimy | P10, P2 |
| A32 | Zasobnik: Pokaż, Zamknij; jedna kopia programu | jest | jest | — |
| A33 | Ekran startowy z fazami, znacznikami API / BOOT / WS, „Wejdź mimo to” po 12 s, granica 40 s | ekran ładowania | przenosimy bramkę gotowości | P3 |
| A34 | Skróty: F10 / `Ctrl+/` tabela, `Ctrl+N`, `Ctrl+L`, `Ctrl+E`, `Ctrl+M`, `Ctrl+,`, `Ctrl+Shift+D/S/I/M`, Alt+←/→, boczne przyciski myszy | własny zestaw z nagrywaniem skrótów | dostosowujemy: brakujące dochodzą, kolizje rozstrzyga rejestr Alfy | P3 |
| A35 | Tabela skrótów: „Na start”, wyszukiwanie bez polskich znaków, filtr obszaru, „Otwórz” | ściąga `Ctrl+/` | dostosowujemy: wyszukiwanie i filtr | P3 |
| A36 | Dymek doku z etykietą programu | własne podpowiedzi Alfy | jest (lepsze) | — |

## B. Ustawienia i panel System

| Nr | Funkcja w Delcie | W Alfie dziś | Decyzja | Grupa |
| --- | --- | --- | --- | --- |
| B1 | „Styl odpowiedzi” (temperatura), „Pamięć rozmowy” 10–1000 wiadomości (domyślnie 250) | brak w oknie | przenosimy | P4 |
| B2 | Kontekst modelu 2K–1M z gotowymi wartościami; najdłuższa odpowiedź 128–65 536; ostrzeżenie o złym zestawie | brak w oknie | przenosimy | P2 |
| B3 | Sposób wczytywania (mmap), dekodowanie spekulatywne, wczytanie przy starcie | brak w oknie | przenosimy | P2 |
| B4 | Wersja silnika, „Sprawdź aktualizacje”, „Aktualizuj teraz” | pakiety silników | przenosimy | P2 |
| B5 | Rozkład modelu GPU / CPU / RAM (role kart, podział, margines, warstwy, układy gotowe, karta przez sieć) | role kart na stronie Urządzenia | przenosimy pełny rozkład | P2 |
| B6 | Karty stanu: silnik rozmowy, silnik obrazów, głos i wizja | Zdrowie systemu | dostosowujemy | P2 |
| B7 | Klucze API: OpenAI, Anthropic, Google, OpenRouter, Groq, Stability, Replicate, Together, Getimg, fal | Konta i klucze API (lepsze: diagnoza, wiek klucza, konto zapasowe) | jest; dochodzą dostawcy obrazów | P8 |
| B8 | Role agentów z dostawcą i modelem | obsada ról, kolejki modeli | jest | — |
| B9 | „Automatyczny dobór modelu”: pula modeli z rolami, polityka, podgląd decyzji | kolejki na klasę zadania | dostosowujemy: podgląd decyzji i pula | P2, P12 |
| B10 | Kolejka awaryjna: do 5 pozycji, przerwa 1 / 5 / 15 / 30 min, stan dostępności | kolejki modeli (do 10), bezpieczniki | jest | — |
| B11 | Monitor: procesor, pamięć, dysk, każda karta (pamięć, obciążenie, temperatura, moc), silnik | strona Urządzenia | przenosimy pełny monitor | P2 |
| B12 | Wygląd: motyw, gęstość (3), czcionka (4), kolor akcentu (8 + własny) | motyw, 3 style, gęstość, powiększenie, akcent | jest; dochodzi skala czcionki | P11 |
| B13 | Zaawansowane: próbkowanie „dobierane do modelu” albo własne (temperatura, top-p, top-k, kara za powtórzenia) | brak w oknie | przenosimy | P4 |
| B14 | Edytor persony z bloków: włącz / wyłącz, kolejność, edycja, własne bloki, licznik, podgląd, eksport do treningu | Kreator agentek (persona, test na sucho) | dostosowujemy: bloki w Kreatorze | P11 |
| B15 | Moduły: 14 przełączników zdolności | poziomy autonomii, strona „Moduły” (wkrótce) | dostosowujemy — łączy się z C4.3 | P7 |
| B16 | Warsztat modułów: narzędzia pisane przez AI, raport bezpieczeństwa, ręczne zatwierdzenie, archiwum | wtyczki Wasm, Ulepszacz, kwarantanna umiejętności | jest (bezpieczniej: piaskownica Wasm) | — |
| B17 | Własne polecenia: frazy, sposób dopasowania (zawiera / zaczyna / dokładnie / wzorzec), akcja, priorytet, tester | reguły Marszałka (inny cel) | przenosimy | P5 |
| B18 | Polecenia wbudowane (16): muzyka, głośność, programy, okna, ekrany, liczenie, quiz, literowanie, losowanie, data | brak | przenosimy | P5 |
| B19 | Wbudowana instrukcja: 9 działów, wyszukiwanie, częste pytania, ścieżka „Gdzie” | przewodnik w dokumentacji | przenosimy do programu | P5 |
| B20 | „Co nowego”: wersja, folder instalatora, aktualizacja, lista zmian | Aktualizacje z historią i wagami | jest (lepsze) | — |
| B21 | Panel System: 4 diody + alarm; podsumowanie Model / Działa na / Warstwy / Szybkość / Wydajność z oceną | stan w belce | przenosimy | P2 |
| B22 | „Korzystanie z modeli”: tylko offline / offline + online / tylko online | profil Lokalny / Hybryda / Chmura | jest; ujednolicić nazwy i skutki | P2 |
| B23 | „Zapas VRAM” 0–512 MiB co 32, „Przeładuj model” | brak | przenosimy | P2 |
| B24 | Karta zintegrowana: lekkie zadania wizji, interfejs na karcie zintegrowanej | lekki dostawca na karcie zintegrowanej | jest; dochodzi przełącznik interfejsu | P2 |
| B25 | Zakładka Łącze: połączenia, liczba narzędzi, zapas, katalog silnika | Zdrowie systemu | dostosowujemy | P2 |
| B26 | Panel System — szczegóły: wersja w nagłówku, licznik kart przy „Sprzęt”, niedostępny wariant silnika wyszarzony, dymek „Gotowy · CUDA · CPU % · RAM % · GPU %” | stan w belce (prawdziwy; w Delcie belka częściowo na sztywno) | przenosimy do panelu System; belka Alfy zostaje | P2 |
| B27 | Nawigacja ustawień jako dok z 9 kaflami | nawigacja Ustawień z wyszukiwaniem, licznikami i klawiaturą | jest (lepsze); dok tylko jako skrót | — |

## C. Rozmowa

| Nr | Funkcja w Delcie | W Alfie dziś | Decyzja | Grupa |
| --- | --- | --- | --- | --- |
| C1 | Karty rozmów: przełączanie, zmiana nazwy dwuklikiem, zamykanie, dodawanie | karty sesji `Ctrl+1…9` | jest | — |
| C2 | Profil karty: zakres pamięci Sesja / Osoba / Projekt, kategoria, właściciel | zakresy pamięci, projekty | przenosimy wybór w karcie | P4 |
| C3 | Archiwum rozmów: stronicowanie, szukanie, grupowanie, 6 filtrów, statystyki | lista sesji z szukaniem, kosz z cofnięciem | przenosimy filtry i statystyki | P4 |
| C4 | Eksport rozmowy: Markdown, JSON, PDF; archiwizacja i przywracanie | eksport `.alfa`, Markdown / HTML | przenosimy JSON i PDF | P4 |
| C5 | Zdarzenia tury: tekst, myślenie, narzędzia, zapas, zmiana modelu, zużycie | strumień zdarzeń z numerami i wznowieniem | jest (lepsze) | — |
| C6 | Zatrzymanie tury z wycofaniem z historii | stop; historia tylko do dopisywania | jest (inaczej: przerwana tura zostaje jako gałąź) | — |
| C7 | Blok „Myślenie AI” (w Delcie wyłączony w kodzie) | szczegóły odpowiedzi | jest | — |
| C8 | Karty wywołań narzędzi: parametry, wynik | kroki narzędzi z cofaniem | jest | — |
| C9 | Przypinanie odpowiedzi do Tablicy; kopiowanie kodu; powiększanie obrazów; etykieta modelu z powodem wyboru | przypinanie paska akcji, kopiowanie | przenosimy Tablicę i powód wyboru modelu | P4 |
| C10 | Markdown z kolorowaniem 12 języków, tabele, obrazy, osadzone wideo | Markdown renderowany w Rust | jest; osadzanie wideo — nie (reguła bezpieczeństwa treści) | — |
| C11 | Załączniki: do 12 plików po 20 MB; 11 rodzajów; przeciągnij i upuść | załączniki, zdjęcie z kamery | przenosimy limity i rodzaje | P4 |
| C12 | Tory załącznika: Wizja / Tekst / RAG / Odtwarzacz, z planem przekazania | brak | przenosimy | P4 |
| C13 | Czytniki: tekst, tabela (CSV), PDF; akcje: do RAG, transkrypcja, odtwórz, pobierz, kopiuj | podgląd artefaktów | przenosimy | P4 |
| C14 | Wyciąganie tekstu: 20 formatów tekstowych + PDF, DOCX, XLSX, ODT, RTF (do 80 000 znaków) | planowane C2.11 | przenosimy (realizuje C2.11) | P4 |
| C15 | Obraz w pytaniu: model widzący lokalnie albo w chmurze | załącznik-zdjęcie; model widzący czeka na C3.6 | przenosimy | P4 |
| C16 | Pasek pola pisania: szukaj w sieci, tryb kodu, pliki, obraz, dyktuj, odczytaj, więcej | pole z `/` i `@`, załącznik, mikrofon | przenosimy brakujące przyciski | P4 |
| C17 | 6 przełączników narzędzi na rozmowę: Internet, Kod, Pliki, Powłoka, RAG, Kreacja | autonomia na sesję | przenosimy | P4 |
| C18 | Panel boczny: Tablica (40 × 20 000 znaków), pytania „z boku” (20), Źródła; sam się otwiera od 1800 px | 7 paneli bocznych | przenosimy | P4 |
| C19 | Tryb skupienia: własny model, prompt, temperatura, limit, licznik czasu i tokenów, eksport | tryb skupienia F11 (sam widok) | przenosimy ustawienia | P4 |
| C20 | Arena: 2 modele równolegle, głosowanie (lewy / remis / prawy / oba złe), tryb ślepy, losowanie, statystyki, historia 50, eksport | brak | przenosimy | P4 |
| C21 | Szablony promptów: 7 kategorii, ok. 20 wbudowanych, własne ze zmiennymi | szablony sesji | przenosimy | P4 |
| C22 | Panel dostawców wg regionu; środki na koncie; kolejność zapasu przeciąganiem | Konta, kolejki z kafelkami | jest; dochodzi widok regionów | P2 |
| C23 | Szybki wybór modelu: filtr, grupy (najlepsze, ostatnie), ocena dopasowania do karty, „Wyładuj model” | wybór profilu | przenosimy | P2 |
| C24 | Dynamiczna długość odpowiedzi (256 / 768 / pełna) | brak | przenosimy | P4 |
| C25 | Budżet kontekstu: rezerwa na odpowiedź ≤ 1/3, streszczenie pominiętych tur (1400 znaków), 250 wiadomości, 50 wywołań na turę, wynik narzędzia do 3000 znaków | kompakcja i limity kroków | dostosowujemy: wartości jako punkt wyjścia | P4 |
| C26 | Automatyczne wyszukiwanie dla modeli bez narzędzi (pogoda, świeże dane; wstrzyknięcie 2400 znaków) | brak | przenosimy, z oznaczeniem treści | P5 |
| C27 | Wybór modułów narzędzi wg intencji; małe modele bez narzędzi, gdy brak intencji | filtr zmyślonych wywołań | przenosimy | P5 |
| C28 | Szyfrowanie treści rozmów i faktów w bazie | szyfrowanie eksportu i profilu głosu | przenosimy szyfrowanie bazy (klucz w Menedżerze poświadczeń) | U |
| C29 | Tłumaczenie PL / EN modelem lokalnym, pamięć 500 wpisów | brak | przenosimy | P5 |
| C30 | Przełącznik „Mózg”: rozmowa przez przepływ | brak | przenosimy z przepływami | P11 |
| C31 | Kafle startowe, „Aktywny kontekst”, pasek multimediów rozmowy | pusty stan z propozycjami | dostosowujemy | P4 |
| C32 | Rozmowa w odczepionym oknie | brak | przenosimy | P3 |
| C33 | Edycja, ponowienie, gałęzie, wyszukiwanie w rozmowie, polecenia `/` | **Alfa ma, Delta nie** | jest | — |

## D. Agentki

| Nr | Funkcja w Delcie | W Alfie dziś | Decyzja | Grupa |
| --- | --- | --- | --- | --- |
| D1 | 4 role wbudowane: planista, programista, recenzent, badacz | role: Dyrygentka, Mówczyni, Strażniczka pamięci, Pisarka, Badaczka, Krytyczka, Myślicielka, Wykonawczyni, Koderka | jest (więcej) | — |
| D2 | Dostawca i model na rolę | obsada + kolejki na klasę zadania | jest | — |
| D3 | Router agentów: słowo lub wzorzec → rola, agent domyślny | zwracanie się po imieniu, Dyrygentka | przenosimy reguły słów | P11 |
| D4 | Przepływ: planista → do 8 podzadań → 3 równolegle → synteza; zapas: jeden agent | delegowanie, zadania równoległe (4), krytyk | jest; dochodzi widok przepływu | P12 |
| D5 | Agent programista — 20 sekcji podglądu polityki, kontekstu, patchy, audytu; nic nie wykonuje | mosty CLI, terminal, Broker | dostosowujemy: w Alfie wykonuje naprawdę | P12 |
| D6 | Indeks semantyczny kodu: fragmenty 1600 znaków, 48 na plik, 8000 razem | wyszukiwanie hybrydowe | przenosimy dla katalogu projektu | P12 |
| D7 | Pamięć per osoba: streszczenie po 6 wiadomościach, fakty poparte słowami użytkownika, 6 kategorii | pamięć 4-warstwowa z zatwierdzaniem | dostosowujemy: kategorie i próg poparcia | P6 |

## E. Narzędzia agentek (59 w Delcie)

| Nr | Narzędzia w Delcie | W Alfie dziś | Decyzja | Grupa |
| --- | --- | --- | --- | --- |
| E1 | Wizja: `look` (opis z kamery + rozpoznanie twarzy) | zdjęcie jako załącznik; `camera.look` w planie | przenosimy | P6, C3.7 |
| E2 | Tożsamość: `who_am_i`, `remember_person`, `note_about_person` | brak | przenosimy | P6 |
| E3 | Sieć: `web_search` (8–12 wyników), `web_fetch` (1 MB, 4000 znaków, 30 odnośników), `deep_research` (5–15 stron), `fetch_favorite_news`, `search_favorites` | brak dostawcy wyszukiwania | przenosimy | P5 |
| E4 | Pliki: czytaj (500 linii), zapisz, lista, szukaj (50), usuń, przenieś, kopiuj, zmień nazwę, utwórz katalog | narzędzia plików przez Brokera z koszem i cofaniem | jest (bezpieczniej) | — |
| E5 | Kod: `execute_code` (Python, PowerShell, cmd, bash; 30 s) | terminal, piaskownica | dostosowujemy | P12 |
| E6 | System: polecenie z listy dozwolonych, procesy, zrzut ekranu, tekst z obrazu, urządzenia, sieć, skan sieci, diagnostyka (zapis / lista / porównanie) | analizator powłoki, ekran | przenosimy brakujące przez Brokera | P7 |
| E7 | Komputer: głośność, uruchom program, okna, schowek, klawisze multimediów | częściowo (UIA) | przenosimy | P7 |
| E8 | Mysz i klawiatura, klik po tekście z ekranu (z próbą „na sucho”) | sterowanie UIA w budowie | przenosimy | P7 |
| E9 | Pamięć: `search_memory`, `search_documents`, `remember_thing`, `search_knowledge`, `teach_command` | narzędzia pamięci | jest; dochodzi `teach_command` | P5 |
| E10 | Karty rozmów: lista, odczyt, przełączenie | brak | przenosimy | P4 |
| E11 | Modele: `download_model`, `list_available_models` | brak | przenosimy (pobranie za zgodą) | P2 |
| E12 | Obrazy: `generate_image`, `create_story` | brak | przenosimy | P8 |
| E13 | Multimedia: szukaj, odtwórz, steruj, „co gra”, lista | brak | przenosimy bez biblioteki | P10 |
| E14 | Pomoc: `delta_help` (instrukcja) | brak | przenosimy | P5 |
| E15 | Samorozszerzanie: pomysł, propozycja i plan modułu | Ulepszacz, propozycje wtyczek | jest | — |
| E16 | Narzędzia MCP przez dołączony Node (Ruflo) | MCP w Rust | jest; Ruflo — nie | — |
