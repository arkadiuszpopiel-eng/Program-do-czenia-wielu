# 17 — Przeniesienie funkcji z Delty AI 0.8.35 (etap P)

Stan na **2026-10-11**, wersja 0.0.28. Decyzja właściciela: Alfa jest programem docelowym; z Delty przenosimy
system rozmowy i mowy oraz wszystkie brakujące elementy — skrupulatnie, połączone z planem Alfy.

Źródło: `Delta_AI_0.8.35_KOMPLET_2026-09-18/03_REPOZYTORIUM_GIT_CZYSTE/Delta-AI-System`. Analiza objęła dwa
przejścia po kodzie (zaplecze Python: 46 grup i 356 tras API, 59 narzędzi agenta; okno: 11 obszarów roboczych,
50 widoków). Niczego nie uruchamiano. Specyfikacja głosu ze stałymi: [`19`](19-specyfikacja-glosu-delty.md).
Macierz pozycja po pozycji: [`20`](20-macierz-delty-czesc-1.md) i [`21`](21-macierz-delty-czesc-2.md).

Nowości z rynku dopisane do grup (R1–R13): [`22-nowosci-2026-10.md`](22-nowosci-2026-10.md).

## Zasady przenoszenia

1. Szkielet Alfy zostaje: moduły z kontraktami, Broker, klucze w Menedżerze poświadczeń, bez Pythona.
   Z Delty bierzemy **zachowanie, progi zdobyte na sprzęcie i ekrany** — napisane od nowa w Rust, w wyglądzie Alfy.
2. Każde działanie w systemie idzie przez Brokera. Wad Delty nie przenosimy: potwierdzenia podawanego przez sam
   model, narzędzi plików z dostępem do całego dysku, tokenu administratora bez hasła, treści z sieci bez oznaczenia.
3. Kontrolka, która w Delcie nic nie robi, w Alfie albo działa, albo jej nie ma.
4. Testy Delty (1741 zaplecza, ok. 1400 okna) są kryteriami odbioru przenoszonych zachowań.
5. Każda grupa: wersje z dziennikiem zmian, przegląd w 3 stylach × 2 motywach, test prawdziwej aplikacji, próba
   na laptopie właściciela.

## Co Alfa już ma (nie przenosimy)

Rozmowa z gałęziami, edycją i ponowieniem (Delta tego nie ma) · agentki, Kreator, umiejętności, wtyczki Wasm ·
zadania w tle · pamięć czterowarstwowa i wyszukiwanie hybrydowe · konta dostawców, kolejki modeli, konto zapasowe ·
mosty CLI · kamery USB · terminal · eksport `.alfa` · aktualizacje z historią zmian · Zdrowie systemu · tłumienie
echa z sygnałem odniesienia i przerywanie głosem · dyktowanie i czytanie na głos · PL/EN, trzy style.

## P1 — rozmowa i mowa

Wniosek z analizy: Alfa ma lepszy szkielet rozmowy (dźwięk natywny, tłumienie echa, dwustopniowe przerywanie,
zapis tego, co użytkownik usłyszał), ale niesprawdzony na żywo i z jednym mechanicznym głosem. Delta ma lepsze
silniki, strojenie ze sprzętu, zabezpieczenia i pełny ekran sterowania. Przenosimy to drugie.

| Element | W Delcie | W Alfie |
| --- | --- | --- |
| Synteza Supertonic 3 | 6 kroków, tempo 1,05, 10 barw (F1–F5, M1–M5), dwa zdania z wyprzedzeniem, zapas: Piper, potem Edge | proces poboczny w Rust na ONNX Runtime za `TtsBackend` |
| 17 gotowych głosów | Supertonic ×11 (w tym „spokojna” 0,95), Piper ×5 (gosia i mc_speech z parametrami szumu), Edge „Zofia” ×1 | `voice-tts-contract/src/voices.rs`; wybór głosu dla każdej agentki |
| Rozpoznawanie Qwen3-ASR 1.7B | `llama-server`, start przy pierwszej wypowiedzi, zwolnienie po 10 min, karta przy ≥ 3,2 GB wolnej pamięci, podpowiedź nazw własnych, klip do 30 s | drugi silnik w `voice-stt-impl`; Whisper zostaje zapasem |
| Zabezpieczenia rozpoznawania | odrzut jakości (poniżej 0,25 s, szczyt poniżej 0,02, mniej niż 120 ms mowy) · bramka Silero (250 ms mowy) · flagi: zły język, powtórzenia, pusty tekst · ponowne rozpoznanie Whisperem · lista zmyślonych fraz · normalizacja polska | nowy strażnik rozpoznawania |
| Bezpiecznik silnika | 3 awarie z rzędu → 30 s przerwy → jedna próba | stan i przycisk „Zamknij bezpiecznik” w oknie |
| Koniec wypowiedzi | Smart Turn v3.2: próba po 250 ms ciszy, ponawianie co 350 ms, najdłużej 2 s; wypowiedź do 25 s; zapas — suwak ciszy 0,4–3 s | w miejsce atrapy w `voice-turn-impl` |
| Słowo wywoławcze | w tekście wypowiedzi: pierwsze 3 słowa, tolerancja jednej litery | działa od razu; model Alfy zostaje opcją |
| Telemetria i diagnostyka | 200 ostatnich próbek, p50 / p95 na silnik; 5 testów mikrofonu i silnika | panel w oknie |
| Ekran „Rozmowa na żywo” | 7 bloków, ok. 30 kontrolek, test głosu i mikrofonu, „Posłuchaj” | strona „Głos” i panel boczny |
| Kula stanu | w belce, jako nakładka, w osobnym oknie | rozszerzenie pigułki głosowej |

Stan po wersji 0.0.43 (2026-10-12):

| Element | Stan | Wersja |
| --- | --- | --- |
| Synteza Supertonic 3 | zrobione; rozgrzewka przy włączeniu mikrofonu | 0.0.30, 0.0.35 |
| 17 gotowych głosów | częściowo: 10 barw Supertonic do wyboru dla każdej agentki; głosy Pipera z parametrami i Edge zostają | 0.0.31 |
| Rozpoznawanie Qwen3-ASR | zrobione, z wyborem karta / Whisper na karcie / procesor według wolnej pamięci karty | 0.0.33, 0.0.38 |
| Zabezpieczenia rozpoznawania | zrobione (strażnik, flagi, lista zmyśleń, zapas) | 0.0.33 |
| Bezpiecznik silnika | zrobione, ze stanem i przyciskiem „Zamknij bezpiecznik” | 0.0.33, 0.0.35 |
| Koniec wypowiedzi | zrobione (Smart Turn w osobnym procesie, 250 / 350 / 2000 ms, ustawienia progu i ciszy) | 0.0.34, 0.0.35 |
| Słowo wywoławcze w tekście | Alfa ma własne rozpoznawanie zwrotu po imieniu (dokładniejsze); dopisana pisownia „Alpha”, „Gamma”, „Betta” | 0.0.35 |
| Telemetria i diagnostyka | zrobione: liczniki strażnika i Smart Turn, „Sprawdź silniki”; pięć testów mikrofonu zostaje | 0.0.35–0.0.41 |
| Ekran „Rozmowa na żywo” | częściowo: karta „Silniki rozmowy” w Ustawieniach i w panelu, pięć ustawień rozmowy, język rozpoznawania, kula stanu z tekstem rozpoznawanym na bieżąco w panelu „Głos” | 0.0.35–0.0.45 |
| Kula stanu | częściowo: w panelu „Głos” (stan słowem, kto mówi, tekst na bieżąco); jako nakładka i w osobnym oknie — do zrobienia | 0.0.45 |

Wyjście z P1 (8 scenariuszy na żywo) czeka na właściciela; rozmowa „na sucho” na prawdziwych silnikach przeszła
(pomiary w [`19`](19-specyfikacja-glosu-delty.md)).

Czego z Delty nie bierzemy: martwych kontrolek („Auto-wyślij”, „Głos” i „Prędkość” przy silnikach offline),
braku przerwania modelu przy zatrzymaniu rozmowy, braku rozwijania liczb (Alfa to ma).

Pomiar z 2026-10-11 ([`15`](15-glos-lokalny.md) §5a) to potwierdza: Supertonic potrzebuje 1–2 s na zdanie, a jest
używalny dzięki 6 krokom i wyprzedzeniu o dwa zdania.

**Wyjście z P1:** 8 scenariuszy z listy Delty na laptopie właściciela — odpowiedź ok. 1 s po ostatnim słowie,
liczby, pauza 1,5 s w środku zdania, wahanie, krótkie „tak / nie”, zdanie po angielsku, przerwanie głosem,
wypowiedź 30 s.

## P2 — modele lokalne i panel System

- **Sklep modeli**: zakładki Polecane / HuggingFace / Polskie / Pobrane; filtry rodzaju, rodziny i rozmiaru;
  ocena „mieści się na karcie” (plik + 1,5 GB ≤ pamięć karty); kolejka pobierania; modele GGUF w częściach;
  archiwum z sumą SHA-256 i przywracaniem.
- **Listy**: polecane (ok. 70 pozycji) i rodziny polskie pobierane na żywo (Bielik, PLLuM, Qra, Trurl, Krakowiak, polka).
- **Rozkład modelu na karty**: role kart (główna / współdzieli / bufor / nieużywana), podział warstwami,
  wierszami albo tensorowo, gotowe układy, karta z innego komputera przez sieć.
- **Dopasowanie do pamięci karty**: zapas 0–512 MiB, pięć stopni awaryjnego startu, dobór kontekstu do luzu.
- **Profile rodzin modeli i poprawki szablonów**: łączenie komunikatów systemowych, spłaszczanie wywołań narzędzi,
  strażnik tur (model nie dopisuje wypowiedzi użytkownika), ponowienie po błędzie szablonu.
- **Parametry silnika**: kontekst, pamięć KV, mmap, flash attention, dekodowanie spekulatywne, wczytanie przy
  starcie, zwalnianie po bezczynności.
- **Warianty silnika** CUDA / Vulkan / ROCm z aktualizacją i samoczynnym wycofaniem.
- **Panel System**: podsumowanie (model, gdzie działa, warstwy, szybkość, ocena wydajności) i zakładki Model /
  Sprzęt / Łącze; przełącznik „Korzystanie z modeli” (tylko offline / offline + online / tylko online).
- **Monitor sprzętu**: wszystkie karty (pamięć, obciążenie, temperatura, moc), pamięć karty per proces.
- **Nowe (R10, R11)**: „Alfa Link” — modele z mocniejszego własnego komputera używane jak lokalne (kanał z etapu D);
  parametr przyspieszenia dla modeli z przewidywaniem kilku słów naraz.

Stan po wersji 0.0.45: zrobione — monitor kart NVIDIA na żywo (0.0.36), ocena „mieści się na karcie” (0.0.39),
nowe silniki głosu w pakietach (0.0.35, 0.0.38), wyszukiwarka, kolejność i podsumowanie miejsca na dysku
w katalogu modeli (0.0.45); reszta listy do zrobienia.

## P3 — nawigacja i powłoka

- **Menu kołowe**: F1 „Rdzeń poleceń” (8 pól), F2 „Macierz narzędzi” (8 grup z podmenu), środkowy przycisk
  myszy; 6 suwaków dostrajania, import i eksport ustawień.
- **Doki w górnej belce**: grupy widoków (pokazują tylko widoki, które już istnieją); „Funkcje” — 12
  przełączników zdolności ze zdaniem stanu i kolorem ryzyka oraz przycisk „Odetnij od komputera”; „Skróty” — własne ulubione widoki; „Okna”.
- **Okna odczepiane**: każdy widok, do 12 okien, gotowe układy (Laptop, 2 monitory, 21:9), profile pracy,
  przenoszenie między monitorami, zawsze na wierzchu.
- **Pulpit kafelkowy**: do 12 kafli, trzy skale (także dotykowa), tryb edycji, 8 zapisanych układów, osobne okno.
- **Wstecz / Dalej**: 50 pozycji, Alt+←/→ i boczne przyciski myszy.
- **Tabela skrótów** (F10) z wyszukiwaniem; **ekran startowy** z bramką gotowości.

Dostosowanie: obecna nawigacja Alfy i jej skróty zostają; menu kołowe, doki i pulpit dochodzą jako dodatkowe
sposoby poruszania się. Reguła o skrótach z AltGr obowiązuje.

Stan po wersji 0.0.43: zrobione — Wstecz / Dalej (0.0.37) i wyszukiwanie w tabeli skrótów (0.0.42); reszta listy
do zrobienia.

## P4 — rozmowa: brakujące funkcje

Karty rozmów z profilem (zakres pamięci: Sesja / Osoba / Projekt) · panel boczny: Tablica (40 przypiętych
odpowiedzi), pytania „z boku”, Źródła · Tryb skupienia · Arena (dwa modele, głosowanie, tryb ślepy, historia) ·
załączniki z torami Wizja / Tekst / RAG / Odtwarzacz i czytnikami tekstu, tabel i PDF · archiwum rozmów
z filtrami i statystykami; eksport Markdown / JSON / PDF · szablony promptów ze zmiennymi · „Styl odpowiedzi”
i parametry próbkowania · budżet kontekstu ze streszczeniem pominiętych tur · wybór modelu z oceną dopasowania.

## P5 — sieć, wiedza, polecenia

Wyszukiwanie w sieci, pobieranie stron z ochroną adresów, głębokie badanie (5–15 stron), automatyczne
wyszukiwanie dla modeli bez narzędzi — treść z sieci zawsze oznaczona jako niezaufana · Wiadomości i Ulubione
strony (RSS) · „Naucz” (kadr z kamery i opis), Wiedza (edycja, konsolidacja, import i eksport) · panel RAG
z progiem trafności · 16 poleceń wykonywanych bez modelu (muzyka, głośność, okna, ekrany programu, nauka:
liczenie, quiz, literowanie, losowanie, data) i własne polecenia z testerem · tłumaczenie PL/EN · wbudowana
instrukcja (9 działów).

## P6 — osoby i tożsamość

Baza osób (fakty z kategorią, źródłem i pewnością; osoba główna; historia rozmów) · rozpoznawanie twarzy i głosu ·
aktywna osoba wybiera pamięć · automatyczne rozpoznawanie nowych osób (domyślnie wyłączone) · narzędzia agentki
„kim jestem” i „zapamiętaj osobę”. ADR 0025 przyjęty przez właściciela 2026-10-11. Wzorce zaszyfrowane, klucz
w Menedżerze poświadczeń, kasowanie na żądanie. Łączy się z kontami z etapu U.

## P7 — komputer

Głośność, uruchamianie programów, lista i przełączanie okien, schowek, klawisze multimediów, mysz i klawiatura,
klik po tekście z ekranu, zrzuty — wszystko przez Brokera, z dziennikiem · menedżer procesów · spis urządzeń
i sieci · ekran uprawnień · praca agentki programistki, która w Delcie była tylko podglądem, w Alfie realnie
przez mosty CLI i Brokera · **nowe (R7)**: „zobacz to okno” — pytanie o zawartość wskazanego okna, z maskowaniem.

## P8 — twórczość

Studio obrazu (stable-diffusion.cpp: SD 1.5, SDXL, FLUX.1 i 2, Chroma; tekst → obraz, obraz → obraz, 9 próbników,
LoRA, serie, profile, kreator promptu) · 7 dostawców obrazów w chmurze z kolejką zapasową i szacunkiem kosztu ·
tryb hybrydowy · Galeria z metadanymi i archiwum · Studio opowieści (komiks, książka, artykuł; stała „biblia”
postaci; eksport) · panel Audio · menedżer kamer (do 3 naraz, okno na kamerę, analiza ciągła).

## P9 — awatar

Pięć trybów (kula, 2D, 2.5D, 3D, modele VRM / GLB) · menedżer modeli · 16 emocji · ruch ust według głośności
i 12 kształtów zsynchronizowanych z zegarem dźwięku · regulator jakości · osobne okno.

## P10 — odtwarzanie multimediów

Odtwarzacz (kolejka, powtarzanie, wideo), most sterowania, narzędzia agentki i polecenia głosowe („włącz”,
„głośniej”, „co gra”). Bez biblioteki i playlist (decyzja właściciela). Źródła: pliki lokalne i załączniki;
pobieranie z serwisów — osobna decyzja przed włączeniem.

## P11 — pozostałe

Edytor przepływów (9 typów węzłów, limity, tryb „mózg rozmowy”) · role agentów z modelem na rolę, reguły
routera, przepływ planista → wykonawcy → synteza · edytor persony z bloków · warsztat własnych modułów
(w Alfie: wtyczki Wasm i Ulepszacz) · laboratorium treningu (zestaw danych, konfiguracja, ocena) · pulpit testowy
(32 testy zaplecza, porównanie przebiegów, raport) · personalizacja (gęstość, czcionka, kolor akcentu).

## P12 — agentki programistki

Zlecenie właściciela z 2026-10-11: Alfa i pozostałe agentki mają umieć pracować nad oprogramowaniem tak, jak
robi to asystent programisty — pisać kod, uruchamiać go, budować i testować programy — rozmawiając ze sobą
w trakcie pracy i dobierając model do trudności zadania. W Delcie był to tylko podgląd bez wykonywania.

| Element | Opis | Co Alfa już ma |
| --- | --- | --- |
| Warsztat projektu | katalog projektu jako obszar roboczy: czytanie, szukanie, edycja plików, polecenia, testy, budowanie, git | terminal, panel Pliki, wersje artefaktów, cofanie kroków |
| **Przycisk „Pełne uprawnienia”** | nadrzędny przełącznik w belce i w polu pisania, jak tryb pełnego dostępu w Claude Code i Codex: po włączeniu agentki nie pytają o żaden krok — piszą, uruchamiają, instalują i budują bez zatwierdzeń. Włącza go wyłącznie człowiek (potwierdzenie w oknie Brokera), dla sesji albo projektu; stan jest stale widoczny (czerwony znak w belce i w pigułce), jedno kliknięcie albo STOP go wyłącza | poziom L4 „Maks” już istnieje („nie pyta o nic poza twardymi blokadami Jądra”) — przycisk jest jego szybkim włącznikiem |
| Tryb zwykły w projekcie | bez przycisku: w obrębie katalogu projektu agentka działa sama, a poza nim, przy kasowaniu, sieci i instalacjach — zgoda w oknie Brokera | poziomy autonomii L0–L3, zakresy katalogów w Brokerze |
| Rozmowa między agentkami | planistka dzieli zadanie, wykonawczynie pracują równolegle i zgłaszają wyniki, krytyczka sprawdza; właściciel widzi ich wymianę i może wejść w słowo | role, delegowanie, zadania równoległe, oś czasu |
| Dobór modelu do zadania | czytanie plików, szukanie i streszczanie — model lekki (lokalny albo tani); planowanie, trudne poprawki, przegląd — model mocny, dopiero gdy trzeba; podniesienie klasy po nieudanej próbie | klasy zadań i kolejki modeli (Rozmowa, Kod, Planowanie, Streszczanie), limity kosztów |
| Budżet i raport | limit kroków, czasu i kosztu na zadanie; po pracy raport: co zmieniono, wynik testów, co nie zostało sprawdzone | limity zadań, samoweryfikacja „Gotowe” |
| Bezpieczniki | treść z plików i z sieci traktowana jako dane, nie polecenia; sekrety nie trafiają do modelu; STOP przerywa wszystko | Broker, kill-switch, filtr sekretów |
| Równoległe przebiegi (R3) | każda agentka w osobnej kopii roboczej repozytorium; panel przebiegów z podglądem zmian | zadania równoległe, oś czasu |
| Model do etapu pracy (R4, R5) | tani model szuka i czyta, mocny pisze, średni sprawdza; praca dzielona tylko wtedy, gdy części są niezależne | klasy zadań i kolejki |
| Jedna rozmowa (R1) i głos (R6) | zwykłe pytanie samo przechodzi w zadanie; głosem można zapytać o postęp, wstrzymać i zatwierdzić | rozmowa głosowa |

Co zostaje także przy włączonym przycisku: każde działanie wykonuje Broker i trafia do dziennika (bez pytania,
ale z zapisem i możliwością cofnięcia kroków); agentki nie mogą same włączyć przycisku ani zmieniać Brokera,
audytu i własnych uprawnień; sekrety nie trafiają do modelu; STOP (`Ctrl+Shift+F12`) działa zawsze. Role Dziecko
i Gość z etapu U nie mają tego przycisku.

## P13 — dyski sieciowe i chmurowe, kopie zapasowe

Zlecenie właściciela z 2026-10-11 (nowa funkcja, w Delcie jej nie było): zaawansowane korzystanie z dysków
sieciowych i chmurowych, z pełnymi uprawnieniami do tworzenia kopii zapasowych.

| Element | Opis | Co Alfa już ma |
| --- | --- | --- |
| Podłączanie magazynów | dyski sieciowe w domu i firmie (SMB / NAS), WebDAV, SFTP, magazyny zgodne z S3, OneDrive, Google Drive, Dropbox; logowanie przez okno dostawcy, bez podawania hasła Alfie | klucze i hasła w Menedżerze poświadczeń; kreator kont |
| Przeglądanie i praca na plikach | magazyny widoczne w panelu Pliki jak zwykłe katalogi; agentki mogą na nich czytać, zapisywać i porządkować — przez Brokera, w granicach nadanych uprawnień | panel Pliki, wersje artefaktów, cofanie kroków |
| Kopie zapasowe | pełne uprawnienia do tworzenia kopii: danych Alfy (rozmowy, pamięć, ustawienia, agentki) i wskazanych katalogów użytkownika; harmonogram, kopie przyrostowe, wersje z rotacją, szyfrowanie po stronie komputera przed wysłaniem, sprawdzanie sum, przywracanie całości albo pojedynczych plików | eksport `.alfa` z szyfrowaniem i kopie z rotacją (moduł `transfer`), harmonogram zadań |
| Kilka celów naraz | zasada 3-2-1: kopia lokalna, na dysku sieciowym i w chmurze; raport po każdej kopii i alarm, gdy kopia się nie udała albo jest stara | Zdrowie systemu, powiadomienia |
| Zasady z rynku (R12) | migawki bez powtórzeń, rotacja dzienna / tygodniowa / miesięczna, okresowe sprawdzanie próbki danych, ćwiczone przywracanie; łączniki z logowaniem u dostawcy (R9) | — |
| Bezpieczniki | kopia nigdy nie kasuje źródła; usuwanie starych wersji tylko według ustawionej rotacji; klucze API i sekrety nie trafiają do kopii jawnie; w sesji prywatnej nic nie idzie do chmury | twarda reguła: sekrety tylko w Menedżerze poświadczeń |

Uwaga do budowy: połączenia z siecią lokalną i z chmurą przechodzą przez strażnika sieci (`lib-netguard`) —
jego zmiany wymagają przeglądu właściciela. Konta z etapu U mają własne magazyny i własne kopie.

## P14 — obowiązki codzienne agentek

Zlecenie właściciela z 2026-10-11 (nowa funkcja): możliwość przypisania agentki do zadań codziennych, takich jak
sprawdzanie poczty.

| Element | Opis | Co Alfa już ma |
| --- | --- | --- |
| Przypisanie obowiązku | ekran „Obowiązki”: wybierasz agentkę, zadanie i porę (codziennie o 8:00, co godzinę, po zdarzeniu); gotowe wzory i własne opisy słowami | zadania w tle, wyzwalacze (czas, nowy plik, ręcznie), reguły Marszałka |
| Poczta | konta IMAP, Gmail i Outlook: przegląd nowych wiadomości, streszczenie, oznaczenie ważnych, szkice odpowiedzi; **wysłanie zawsze po Twoim zatwierdzeniu**, chyba że sam zezwolisz dla wybranych nadawców | Broker i zgody; hasła w Menedżerze poświadczeń |
| Kalendarz i przypomnienia | plan dnia rano, przypomnienia głosem i powiadomieniem, wykrywanie kolizji terminów | powiadomienia, głos |
| Inne wzory | przegląd wiadomości z ulubionych stron (P5), porządki w Pobranych, kopia zapasowa (P13), raport kosztów, sprawdzenie stanu systemu, pilnowanie strony albo ceny | Ulepszacz, Zdrowie systemu |
| Budżet i model | każdy obowiązek ma limit kosztu i czasu; domyślnie model lekki albo lokalny, mocny tylko gdy zadanie tego wymaga (jak w P12) | limity zadań, „zadania w tle tylko lokalnie” |
| Raport | po każdym przebiegu krótki wpis w osi czasu i w raporcie dnia: co zrobiono, co czeka na decyzję; możliwość odczytania głosem | raport dnia, oś czasu |
| Rutyny (R2) i poranny przegląd (R8) | obowiązek to zapisane polecenie + zakres + łączniki, uruchamiane z harmonogramu, zdarzeniem albo ręcznie; rano raport: poczta, kalendarz, zadania, koszty | wyzwalacze, raport dnia |
| Bezpieczniki (R13) | treść wiadomości to dane, nie polecenia; wysyłka po zatwierdzeniu; nowy adresat zawsze z pytaniem; obowiązek nie ma dostępu do płatności ani kluczy i nie podnosi sobie uprawnień; przycisk wstrzymania wszystkich obowiązków | Broker, tagi prywatności |

Stan po wersji 0.0.45: cztery wzory obowiązków (0.0.40, ukryte w 0.0.44) **wróciły**. Zadanie w tle uruchomione
wyzwalaczem albo harmonogramem dostaje razem z poleceniem arkusz faktów z programu — otwarte zadania, zadania
zakończone dziś, wyzwalacze z terminami, koszty z dziś i z miesiąca z limitem, katalog roboczy — z zakazem wyjścia
poza te dane (`crates/app-tasks/src/facts.rs`, zbiera `CoreHost::facts` w `crates/app-core/src/host.rs`). Zadania
z sesji prywatnych nie trafiają do przebiegów spoza nich; zadanie oparte na treści niezaufanej arkusza nie dostaje.
Próba na lokalnym modelu (Bielik 4.5B, profil tymczasowy): „Poranny przegląd dnia” w 85 s i „Podsumowanie dnia”
w 3 min 8 s — raporty zgodne z danymi; „Kontrola kosztów” po doprecyzowaniu polecenia — 30 s, trzy punkty
zgodne z danymi. Próba ujawniła i pozwoliła naprawić błąd tworzenia drugiego wyzwalacza o tej samej nazwie po
restarcie programu.

Zostaje w P14: koszty z poszczególnych dni (do porównań tygodni), krótszy krok sprawdzania odpowiedzi przez drugą
agentkę na lokalnym modelu (dziś to większość czasu obowiązku), ekran „Obowiązki”, poczta, kalendarz, budżet
obowiązku i raport z wykonania. Zmiana poleceń wzorów wymaga powtórzenia próby na modelu
(`apps/desktop/ui/src/lib/logic/duties.ts`).

Konta z etapu U mają własne obowiązki i własne skrzynki; telefon i panel WWW pokazują raporty i prośby o decyzję.

## Nie przenosimy

- Rój Ruflo — Alfa ma własne agentki i MCP.
- Wykrywanie środowiska Pythona i skryptowe instalatory silników.
- Klucze w plikach.
- Menedżer plików ograniczony do katalogu wysyłek — Alfa ma panel Pliki.
- Rzeczy w Delcie tylko zapowiedziane (klonowanie głosu Chatterbox, pełny dupleks, śledzenie twarzy dla awatara)
  trafiają do etapu H jako kandydaci; port na Linuksa Alfa planuje w etapie G.

## Weryfikacja

- Dla każdej pozycji macierzy: test przepisany z Delty albo nowy; brak testu oznacza pozycję niezaliczoną.
- Bramki Alfy przy każdej wersji; pomiary na laptopie zapisywane w dokumencie grupy.
- Po każdej grupie porównanie z macierzą: lista pozycji zrobionych, odłożonych i odrzuconych z powodem.
