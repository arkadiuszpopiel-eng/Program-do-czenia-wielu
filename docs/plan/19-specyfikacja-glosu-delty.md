# 19 — Specyfikacja systemu rozmowy i mowy Delty AI 0.8.35 (wzorzec dla P1)

Stan na **2026-10-11**. Dokument zapisuje, **jak dokładnie działa głos w Delcie** — stałe, progi, kolejność
kroków — żeby przeniesienie do Alfy (zadanie P1) niczego nie zgubiło. Źródło: kod Delty (ścieżki względem
`Delta-AI-System/`), czytany, nie uruchamiany. Skróty: `B/` = `backend/`, `F/` = `frontend/src/lib/`.

Kolumna „Alfa” mówi, co robimy: **bierzemy** (wartość z Delty), **zostaje Alfa** (mamy lepsze), **nie bierzemy**.

## 1. Przebieg jednej tury rozmowy

| Krok | Delta | Alfa |
| --- | --- | --- |
| Start rozmowy | mikrofon → 600 ms kalibracji tła → rozgrzanie silnika rozpoznawania (ponawiane co 240 s) | bierzemy kalibrację i rozgrzewanie |
| Początek mowy | głos aktywny ponad 220 ms bez przerwy; nagranie obejmuje 700 ms sprzed wykrycia | bierzemy 220 ms; zapas wstecz 700 ms (dziś 200 ms) |
| Wykrycie głosu | próg = ograniczenie((tło × 1,5 + 0,015) × bramka / 8; 0,026…0,28); tło śledzone średnią 0,97 / 0,03; analiza co 80 ms | zostaje Alfa (Silero + redukcja szumów); próg Delty jako zapas bez modelu |
| Koniec wypowiedzi | po 250 ms ciszy pytanie do Smart Turn; „skończone” → od razu; „nie” → ponawianie co 350 ms, najdłużej max(2000 ms, suwak ciszy); błąd modelu → sam suwak | bierzemy w całości |
| Najdłuższa wypowiedź | 25 s (zakres 5–29 s), potem wysyłka pierwszej części i dalsze słuchanie | bierzemy |
| Rozpoznanie | całe nagranie WAV 16 kHz mono → silnik → zabezpieczenia (rozdz. 3) | bierzemy |
| Słowo wywoławcze | jeśli włączone: musi paść w pierwszych 3 słowach tekstu; dalej idzie reszta zdania | bierzemy |
| Odpowiedź modelu | zwykła tura rozmowy + stała wskazówka „odpowiadaj naturalnie, krótko i konkretnie” (do 4096 znaków) | bierzemy wskazówkę; tura przez rdzeń Alfy |
| Mowa | tekst dzielony na zdania, 2 zdania syntezowane z wyprzedzeniem, odtwarzane kolejno | **jest** — sprawdzone 2026-10-11: serwis mowy Alfy syntezuje naprzód do kolejki 16 fragmentów |
| Przerwanie głosem | głośność > 1,7 × próg przez 350 ms podczas mówienia → stop mowy i modelu | zostaje Alfa (tłumienie echa, potwierdzenie 200–350 ms, słowa „stop / czekaj”) |
| Zatrzymanie rozmowy | Delta nie przerywa modelu | nie bierzemy — Alfa przerywa wszystko |

Stany widoczne dla użytkownika: Gotowy · Słucham · Analizuję · Odpowiadam. Komunikaty: „Nie dosłyszałam —
powtórz trochę głośniej” (znika po 3 s), „Długa wypowiedź — wysyłam pierwszą część, mów dalej…”.

Limity czasu: pytanie o koniec wypowiedzi 1,5 s · transkrypt częściowy 4 s · rozpoznanie końcowe 60 s.

## 2. Rozpoznawanie mowy

### Silnik główny: Qwen3-ASR 1.7B (`B/inference/asr_server.py`)

| Parametr | Wartość |
| --- | --- |
| Pliki | `Qwen3-ASR-1.7B-Q8_0.gguf` (2,17 GB) + `mmproj-Qwen3-ASR-1.7B-Q8_0.gguf` (356 MB), repozytorium `ggml-org/Qwen3-ASR-1.7B-GGUF`, licencja Apache-2.0 |
| Uruchomienie | `llama-server` na 127.0.0.1:8082, kontekst 4096, `--parallel 1`, `--jinja`, `--no-warmup`; wątki max(4, rdzenie / 2) |
| Karta czy procesor | karta, gdy wolne ≥ 3,2 GB pamięci albo nie da się zmierzyć; inaczej procesor (`--no-mmproj-offload`) |
| Start | przy pierwszej wypowiedzi; sprawdzenie awarii po 0,4 s; gotowość odpytywana co 0,25 s do 120 s |
| Zwolnienie | po 600 s bezczynności (sprawdzane co 30 s) |
| Zapytanie | rozmowa z jednym komunikatem: dźwięk WAV w base64 + opcjonalny tekst; temperatura 0, do 512 tokenów |
| Podpowiedź nazw | „Nazwy własne, które mogą paść: …” — najwyżej 24 terminy |
| Długość klipu | ostatnie 30 s nagrania |
| Wynik | `language Polish<asr_text>…` — język wykrywany z odpowiedzi (mapa 31 języków) |

### Zapas: faster-whisper → w Alfie whisper.cpp

Domyślnie `large-v3-turbo`; bez karty NVIDIA modele większe niż `small` są zamieniane na `base`. Łańcuch:
żądany → medium → base. Dekodowanie: wiązka 5, bez filtra ciszy, bez warunkowania poprzednim tekstem, progi
`no_speech` 0,6, `log_prob` −1,0, `compression_ratio` 2,4. Alfa ma już `whisper-server`; bierzemy progi.

### Transkrypt w trakcie mówienia

Co 1200 ms (zakres 500–5000), na procesorze nie częściej niż co 3000 ms; pokazywane ostatnie 90 znaków; nie
przechodzi przez zabezpieczenia i nie trafia do rozmowy.

### Pomiar na laptopie właściciela (2026-10-11)

Model Qwen3-ASR z katalogu Delty uruchomiony silnikiem `llama-server` z Delty (wydanie 11022) na karcie RTX 4050;
40 nagrań: te same 20 zdań głosem Supertonic 3 (z `alfa-supertonic`) i Piperem.

| Miara | Wynik |
| --- | --- |
| Start serwera z modelem | 4 s |
| Czas rozpoznania zdania | mediana 0,44 s, najwolniejsze 5 %: 0,87 s |
| Pamięć karty zajęta w trakcie | ok. 3,4 GB (cała karta, razem z pulpitem) |
| Wykryty język | „Polish” we wszystkich 40 nagraniach |
| Błędne słowa w zdaniach bez liczb i skrótów | Supertonic: 13 ze 120 · Piper: 14 ze 120 — w obu głównie dwa łamańce językowe |

Błędy poza łamańcami: przy Supertonicu 3 słowa („Alpha”, „Hm”, „Żeszow”), przy Piperze 5 (m.in. „Czy mogę” zamiast
„W czym mogę”, „pik” zamiast „plik”). To pomiar zrozumiałości dla maszyny, nie ocena brzmienia. Zdania z liczbami
model zapisuje słowami („dziesięć minut”), więc porównanie z cyframi w tekście nic nie mówi — pominięte.

**Blokada zdjęta w 0.0.32:** silnik `llama-server` w wydaniu 6710 nie znał architektury tego modelu („unknown
model architecture: qwen3vl”). Katalog Alfy pobiera teraz oficjalne wydanie b11559, sprawdzone z tym modelem
(0,43 s na zdanie). Kto ma zainstalowany stary silnik, musi pobrać go ponownie w Ustawieniach.

## 3. Zabezpieczenia rozpoznawania (kolejność jak w `B/routers/stt_router.py`)

| Nr | Zabezpieczenie | Reguła |
| --- | --- | --- |
| 1 | Bramka mowy Silero v5 | okna 512 próbek; mowa, gdy okna z prawdopodobieństwem > 0,5 dają ≥ 250 ms; przy błędzie przepuszcza |
| 2 | Odrzut jakości | nagranie < 0,25 s, szczyt < 0,02 albo < 120 ms aktywnego sygnału (ramka 20 ms, próg max(0,01; 2,5 × 10. percentyl)) → pusty wynik z powodem |
| 3 | Bezpiecznik | rozdz. 4 |
| 4 | Flaga „zły język” | wykryty język inny niż żądany i spoza {pl, en} → tekst kasowany |
| 5 | Flaga „powtórzenia” | to samo słowo ≥ 4 razy z rzędu albo jedno słowo ≥ 60 % z co najmniej 8 słów |
| 6 | Flaga „pusto mimo mowy” | mowa ≥ 2000 ms i pusty tekst |
| 7 | Flaga „za krótko” | mowa ≥ 5000 ms i najwyżej 1 słowo |
| 8 | Ponowne rozpoznanie | dowolna flaga → to samo nagranie przez Whisper; niepusty wynik wygrywa |
| 9 | Lista zmyśleń | znane frazy-halucynacje usuwane |
| 10 | Normalizacja polska | poprawne nazwy własne i wielka litera na początku zdania; liczby bez zmian |

Wszystkie dziesięć bierzemy. W Alfie dochodzi jedenaste: rozwijanie liczb i dat w mowie (już jest).

## 4. Bezpiecznik silnika rozpoznawania (`B/inference/asr_guard.py`)

Stany: zamknięty → otwarty → półotwarty. Otwiera się po 3 awariach z rzędu (zakres 1–20). Po 30 s (zakres
5–3600) przepuszcza dokładnie jedną próbę; sukces zamyka i zeruje licznik, porażka otwiera ponownie. Gdy otwarty,
rozpoznaje Whisper. W oknie: stan, „awarie z rzędu 0/3 · otwarć N”, przycisk „Zamknij bezpiecznik”.

Telemetria: pierścień 200 próbek bez tekstu (silnik, długość dźwięku, czas, stosunek czasu, flagi, zapas);
na silnik: liczba, p50 i p95 czasu oraz stosunku czasu, ostatnie 20.

## 5. Koniec wypowiedzi: Smart Turn (`B/inference/turn_detect.py`)

| Parametr | Wartość |
| --- | --- |
| Model | `pipecat-ai/smart-turn-v3`, plik `smart-turn-v3.2-cpu.onnx` (ok. 9 MB), licencja BSD-2-Clause; procesor, wątki min(4, rdzenie) |
| Wejście | ostatnie 8 s, dopełnione z lewej do 128 000 próbek, normalizacja do średniej 0 i odchylenia 1 |
| Cechy | log-mel jak w Whisperze: okno 400, krok 160, 80 pasm, obcięcie do max − 8, potem (x + 4) / 4 → tablica 1 × 80 × 800 |
| Decyzja | „skończone”, gdy prawdopodobieństwo > 0,5 |
| Strojenie | jeśli ucina: próg 0,6–0,7 albo pierwsza próba po 350 ms; jeśli zwleka: próg 0,4 |

W Alfie zastępuje atrapę tekstową w `voice-turn-impl`. Test zgodności cech z wzorcem przepisujemy z Delty.

## 6. Synteza mowy

| Silnik | Jak działa w Delcie | Parametry | Alfa |
| --- | --- | --- | --- |
| **Supertonic 3** | ONNX Runtime na procesorze | 6 kroków, tempo 1,05, język `pl`, wątki min(4, rdzenie), barwy F1–F5 i M1–M5, 44,1 kHz | bierzemy jako głos główny; proces poboczny |
| **Piper** | pakiet Pythona | głosy `gosia-medium`, `mc_speech-medium`, `darkman-medium`; długość = 1 / tempo (0,4–2,0); `noise_scale`, `noise_w` | Alfa ma Pipera; dochodzą głosy i parametry |
| **Edge** | usługa Microsoftu przez sieć | `pl-PL-ZofiaNeural`; tempo jako procent | opcja wyłączona domyślnie, nigdy w sesji prywatnej |
| Kokoro-82M | — | brak polskiego | nie bierzemy |

Kolejność zapasów: Supertonic niegotowy → Piper → Edge (jeśli włączony). Wyjątek silnika lokalnego → następny.

### 17 gotowych głosów

| Głos | Silnik i barwa | Tempo | Szum / rytm |
| --- | --- | --- | --- |
| Naturalna F1 (domyślna), F5, F2, F3, F4 | Supertonic | 1,05 | — |
| Męski M1…M5 | Supertonic | 1,05 | — |
| Spokojna (F1, wolniej) | Supertonic F1 | 0,95 | — |
| Naturalna | Piper gosia | 1,00 | 0,667 / 0,80 |
| Energiczna / młoda | Piper gosia | 1,12 | 0,72 / 0,85 |
| Spokojna / ciepła | Piper gosia | 0,90 | 0,60 / 0,75 |
| Klara — jasna | Piper mc_speech | 1,00 | 0,667 / 0,80 |
| Klara — dojrzała | Piper mc_speech | 0,92 | 0,60 / 0,72 |
| Zofia — online HD | Edge | 1,00 | — |

W Alfie nazwy bez „Delta —”; każda agentka dostaje własny głos domyślny z tej listy.

### Tekst do mowy i kolejka

Czyszczenie: usuwane emoji, bloki i wstawki kodu oraz znaki `*_~#>[]()`. Koniec zdania: `! ? …`, nowy wiersz
albo kropka nie przy cyfrze i nie po skrócie ani pojedynczej literze. Zdanie do 600 znaków; krótsze niż 2 znaki
pomijane. Dwa zdania w syntezie z wyprzedzeniem. Pamięć podręczna: 100 nagrań, klucz z tekstu, silnika, głosu
i tempa. Alfa ma własny podział na zdania i normalizację — bierzemy wyprzedzenie i rozmiar pamięci.

### Ruch ust

12 kształtów (aa, ee, ih, oh, oo, mbp, fv, td, kg, sh, ch, cisza); samogłoski z wagą 0,92, reszta 0,72. Bez
danych z silnika: 78 ms na literę / tempo, najmniej 120 ms, przerwa 45 ms / tempo, całość skalowana do długości
nagrania. Zegar: czas odtwarzania dźwięku; przejście między kształtami 28 ms. Wchodzi razem z awatarem (P9).

## 7. Rozpoznawanie mówiącego

Model WeSpeaker ResNet34 (ONNX): 80 pasm, okno 25 ms, krok 10 ms. Dopasowanie: podobieństwo do wzorca osoby
≥ 0,62 i przewaga ≥ 0,06 nad drugim wynikiem. Trafienie ustawia aktywną osobę na 15 s. Alfa ma `voice-speaker`
z zaszyfrowanym profilem; progi i powiązanie z osobami w P6.

## 8. Ekran „Rozmowa na żywo” — kontrolki

| Blok | Kontrolka | Zakres i wartość domyślna |
| --- | --- | --- |
| Nagłówek | kula stanu, znaczniki VAD / TTS / MIC, stan sesji z procentem poziomu, „Rozpocznij rozmowę” | — |
| Głos AI | język rozpoznawania | pl, en, de, fr, es, it, uk, ja, zh; domyślnie pl |
| | głos, tempo | tempo 0,5–2,0 co 0,05; domyślnie 1,05 |
| | „Auto-czytaj odpowiedzi” | domyślnie wyłączone; w rozmowie na żywo włączone |
| | „Test głosu”, „Stop” | zdanie próbne |
| | Synchronizacja ust | próbki, p50, p95, skoki zegara |
| Mikrofon | urządzenie | zapamiętywane |
| | bramka szumów | 1–30, domyślnie 6 |
| | cisza kończąca | 0,4–3,0 s co 0,1; domyślnie 1,0 |
| | Smart Turn, bramka mowy Silero, transkrypt na żywo | wszystkie włączone |
| | miernik poziomu, „Test mikrofonu” | test kończy się po 10 s |
| Diagnostyka | 5 testów: uprawnienie mikrofonu, stan silnika, częstotliwość, liczba mikrofonów, obsługa mowy | — |
| Telemetria | tabela na silnik, stan bezpiecznika, „Zamknij bezpiecznik”, ostrzeżenie o wyłączonym tłumieniu echa | — |
| Silniki | silnik mowy, barwa, 17 kart głosów z „Posłuchaj” | Supertonic 3, F1 |
| | silnik rozpoznawania, model zapasowy | Qwen3-ASR; Whisper tiny…large-v3 |
| | słowo wywoławcze i jego treść | wyłączone; w Alfie imię agentki |
| Zaawansowane | przełącznik rozmowy, informacja o silnikach | — |

W Alfie każda kontrolka działa od razu (w Delcie część dopiero od następnego startu rozmowy), a trzy martwe
pozycje Delty („Auto-wyślij”, „Głos” i „Prędkość” przy silnikach offline) znikają albo zaczynają działać.

## 9. Kryteria odbioru P1

Testy przepisane z Delty: bezpiecznik (otwarcie po 3., jedna próba po czasie), flagi jakości, bramka mowy, odrzut
ciszy i szumu, cechy Smart Turn zgodne z wzorcem, wybór silnika i zapasów, głosy i ich parametry, podział na
zdania, transkrypt częściowy 1200 / 3000 ms. Próba na laptopie: 8 scenariuszy z
`TEST_GLOSU_NA_ZYWO_2026-09-11.md` (w Delcie nigdy nie wykonana na żywo — zabrakło mikrofonu).

Znane ograniczenia Delty, które przechodzą na Alfę do sprawdzenia: Smart Turn ocenia melodię zdania i może uciąć
urwane zdanie; na 6 GB rozpoznawanie bywa na procesorze (2–4 s); Supertonic nie podaje czasu głosek.
