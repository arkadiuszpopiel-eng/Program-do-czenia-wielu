# Voice Suite — system głosowy programu Alfa

> Dokument pochodny od `docs/PLAN.md` (§6, §3.4–3.5, §13, §16.2). Nie wprowadza nowych decyzji; wartości oznaczone „do pomiaru w F0 / Voice Lab" są hipotezami, nie ustaleniami. Wszystkie oceny jakości polskiego w STT/TTS pochodzą ze źródeł wtórnych i wymagają odsłuchu / pomiaru na korpusie własnym.

## 1. Cele

| Cel | Znaczenie |
|---|---|
| Rozmowa pełnodupleksowa | jak z człowiekiem: agentka mówi, a Ty możesz w każdej chwili wejść jej w słowo |
| Polski jako język pierwszy | mieszany PL/EN (nazwy własne, kod, terminy); auto-wykrywanie PL/EN w STT |
| Cztery odrębne głosy | Alfa, Beta, Gama, Delta — głos idzie za personą (biblie głosu w `docs/PERSONAS.md`) |
| Przerwanie w każdej chwili | zatrzymanie dwustopniowe + zrozumienie, co Ty słyszałeś i co chciałeś poprawić |
| Lekko i lepiej | działa na CPU (profil A, baseline), lepiej na GPU/chmurze (B/C/D) |
| Wymienialność | każdy element to moduł z kontraktem (`-contract` / `-impl` / `-fake`) |

Zasady projektowe, które dotyczą głosu wprost (PLAN §2): przerywalność wszędzie, fallback na każdej ścieżce, mierzalność (każdy cel ma metrykę i próg go/no-go), prywatność przez tagi sesji (dotyczy też chmurowego głosu).

## 2. Moduły `voice-*`

Każdy moduł = trójka crate'ów (PLAN §3.2), manifest `module.toml` z budżetem RAM/CPU, cyklem życia (moduły głosowe: ładowane przy pierwszym użyciu, zwalniane po bezczynności — konfigurowalnie) i izolacją. Audio i DSP nigdy nie przechodzą przez Wasm/IPC w callbacku audio.

| Moduł | Rola | Domyślnie / kandydaci | Izolacja | Fala |
|---|---|---|---|---|
| `voice-audio` | wejście/wyjście, hot-plug, wybór urządzeń, mikser, ducking, normalizacja głośności, routing per agentka | crate `wasapi` (loopback, tryb zdarzeniowy, per-proces), wątek RT; `cpal` odrzucony (brak loopbacku) | in-proc | F2 |
| `voice-dsp` | AEC, redukcja szumu, AGC | AEC z własnym strumieniem TTS jako referencją (crate `aec3` / `sonora`); awaryjnie loopback procesowy (Win10 20348+) lub tryb Communications; NS: RNNoise (DeepFilterNet3 opcja) | in-proc | F2 |
| `voice-vad` | wykrywanie mowy | Silero VAD (MIT) / TEN VAD | in-proc | F2 |
| `voice-turn` | koniec tury | Smart Turn v3.2 (PL wśród 23 języków, ~8 MB, ~10 ms CPU) + polityka cierpliwości (hezytacje „yyy", regulowana) | in-proc | F2 |
| `voice-stt` | rozpoznawanie mowy | whisper.cpp (large-v3 / turbo; CUDA/Vulkan/CPU), Parakeet v3 (ONNX), chmura (ElevenLabs Scribe v2 RT, Soniox, gpt-4o-transcribe, Qwen3-ASR); tryb dwuprzebiegowy (szybki partial + dokładny final), biasing/hotwords, auto PL/EN, pewność | proces | F2 |
| `voice-speaker` | weryfikacja właściciela, opcjonalnie diaryzacja | WeSpeaker / ECAPA przez sherpa-onnx (CPU); progi zależne od ryzyka akcji | in-proc | F5 |
| `voice-wake` | PTT, adresowanie, słowa wywoławcze | **v0 (F2):** PTT / przełącznik (hook `WH_KEYBOARD_LL`) + adresowanie po imieniu z transkryptu. **v1 (F5):** opcjonalne „Hej Alfa/Beta/Gama/Delta" (openWakeWord tylko EN → własny trening na mowie syntetycznej + Twojej; włączane dopiero po spełnieniu FAR/FRR), tryb „zawsze słucham" z bramką właściciela, „nie przeszkadzać" | in-proc | F2 (v0) / F5 (v1) |
| `voice-dialog` | automat tur, barge-in, backchannel, intencje przerwań, usłyszany prefiks, wznawianie, mowa proaktywna | §5–§8 tego dokumentu | in-proc | F2 |
| `voice-tts` | synteza | lokalnie (baseline): Pocket TTS + model PL społeczności (CPU, RTF ≈ 0,21, ~200 ms do 1. fragmentu, klon z krótkiej referencji), Piper pl_PL (zapas); poza baseline (profil D): Chatterbox Multilingual, XTTS-v2, F5 (CUDA / stabilny ROCm), VibeVoice-Realtime (do sprawdzenia); chmura: ElevenLabs, Cartesia, Google Chirp 3 HD, Azure pl-PL, Gemini TTS, OpenAI, MiniMax Speech 2.8 (PL + klon); chunker, streaming, dostawca znaczników słów, cache fraz stałych, łańcuch fallback per agentka | proces | F2 |
| `voice-persona` | biblia głosu per agentka, styl → silnik, planista emocji, słownik wymowy, normalizator PL, podział mówione/ekranowe | §10 tego dokumentu, `docs/PERSONAS.md` | in-proc | F2 |
| `voice-cmd` | szybka ścieżka komend bez LLM | „stop", „pauza", „głośniej/ciszej", „wycisz mikrofon", „przełącz na Deltę", „powtórz", „wznów", „anuluj"; keyword-spotter „stop/czekaj" | in-proc | F2 |
| `voice-dictation` | dyktowanie do dowolnej aplikacji | STT → wstrzyknięcie tekstu (UIA / SendInput / schowek), komendy interpunkcji, profile per aplikacja | in-proc | F5 |
| `voice-readaloud` | czytanie na głos zaznaczenia / schowka / strony | UIA `TextPattern.GetSelection` (zapas: Ctrl+C), kolejka, tempo, podświetlanie | in-proc | F5 |
| `voice-lab` | benchmarki i casting głosów | opóźnienia p50/p95, echo, fałszywe przerwania, WER na korpusie własnym, dokładność prefiksu, ślepe A/B, proxy MOS, edytor słownika | proces | F0 (narzędzie pomiarowe) |
| `voice-s2s` *(P1, opcjonalny)* | natywny speech-to-speech w chmurze („tryb szybkiej rozmowy") | OpenAI Realtime / Gemini Live (PL deklarowany; lokalnego S2S dla PL brak); głosy = presety dostawcy przypisane do agentek, nie własne biblie | proces | F5 |
| `voice-transcribe` *(P2)* | nagrania / spotkania z diaryzacją | pyannote community-1 (offline); komunikat o zgodzie osób trzecich | proces | poza v1 |

Ograniczenia z PLAN §7.3, które dotyczą głosu: PTT globalny wymaga hooka `WH_KEYBOARD_LL` (zwykły skrót nie zgłasza puszczenia klawisza); hooki, PTT i dyktowanie przez SendInput **nie działają, gdy na pierwszym planie jest okno administratora** bez helpera `uiAccess` — UI komunikuje to wprost.

## 3. Profile potoku A–D

Profil wybiera `device-profile` **per maszyna** (nakładka `config/machine/<id>.toml`), użytkownik może nadpisać w „Kreatorze sprzętu". Finalny wybór silników per profil rozstrzyga Voice Lab.

| Profil | VAD / koniec tury | STT | „Mózg" rozmowy | TTS | Uwagi |
|---|---|---|---|---|---|
| **A. Minimum lokalne** (musi działać na baseline) | Silero + Smart Turn v3.2 (CPU) | whisper.cpp Vulkan, large-v3-turbo Q5_0 (547 MiB) z bramką VAD; fallback small Q5 / CPU | opcjonalnie lokalny 3–4B Q4 (llama.cpp Vulkan) do komend i fallbacku | Pocket TTS PL (CPU) — v0: 4 wbudowane głosy zróżnicowane wysokością i tempem, bez kluczy; docelowo klon z referencji po castingu / Piper pl_PL jako zapas | zero chmury; jakość rozmowy ograniczona rozmiarem LLM; opóźnienia best effort |
| **B. Hybrydowy** (rekomendowany domyślny) | jw. | lokalny (Vulkan) albo chmurowy | chmurowy model przez API (Opus 5.5 / GPT-6 Sol / inne) | Pocket-PL lokalnie **albo** chmurowy (ElevenLabs / Cartesia) | najlepszy kompromis; lokalne STT chroni audio |
| **C. Jakość-chmura** | jw. | Scribe v2 RT / Soniox | API | ElevenLabs / Cartesia; opcjonalnie S2S (Realtime / Gemini Live) | najniższe opóźnienie i najlepszy PL; audio w chmurze (tag prywatności) |
| **D. Mocny lokalny** | jw. | **D-AMD16:** whisper.cpp Vulkan, large-v3 pełny. **D-CUDA:** whisper.cpp CUDA, turbo Q5 | **D-AMD16:** lokalny LLM 8–14B Q4 (Vulkan) i/lub API. **D-CUDA:** 3–4B lokalnie + API | **D-CUDA:** Chatterbox / XTTS-v2 tylko jeśli mieszczą się w 6 GB obok STT (rezydencja na przemian), inaczej Pocket-PL na CPU. **D-AMD16:** Pocket-PL (CPU); Chatterbox / XTTS / F5 dopiero po dodatnim spike'u PyTorch-ROCm / portu Vulkan na RDNA4 | wysoka jakość lokalnie tam, gdzie sprzęt pozwala |

### 3.1 Profile a maszyny

| Maszyna | Sprzęt | Ścieżka GPU | Profile dostępne | Ograniczenia |
|---|---|---|---|---|
| **Baseline** (minimum, brak fizycznie) | Ryzen 5 5600 · RX 7600 8 GB (RDNA3) · 16 GB | Vulkan, bez CUDA | A / B / C | emulacja na Twoich maszynach: 6 rdzeni, 16 GB, VRAM 8 GB w zarządcy rezydencji; czasy GPU z desktopu × ~2,2; do wyników CPU +20–30% (5700X3D ma 96 MB L3 vs 32 MB) |
| **Desktop** (Standard-AMD) | Ryzen 7 5700X3D · RX 9070 XT 16 GB (RDNA4) · 32 GB | Vulkan | A / B / C / D-AMD16 | brak CUDA; HIP/ROCm dla RDNA4 na Windows do sprawdzenia w F0 (`ErrorDeviceLost` zgłaszany w części narzędzi) |
| **Laptop** (Laptop-CUDA) | i7-13700H · RTX 4050 6 GB · 16 GB | CUDA | A / B / C / D-CUDA | tylko 6 GB VRAM (mniej niż baseline): STT i ciężki TTS nie są rezydentne naraz; tryb baterii i limit termiczny; mocny CPU (Pocket-PL komfortowy) |
| Przyszły „Mocny" | NVIDIA ≥ 12–16 GB | CUDA | pełny lokalny stos (D) | — |

Budżety pamięci (PLAN §3.4, do zmierzenia w F0): RAM z załadowanym głosem ~2,5–4 GB łącznie (WebView2 300–500 MB + whisper Q5 1–1,5 GB + Pocket TTS 0,5–1,5 GB + reszta), zapas ≥ 7 GB z 16; VRAM 8 GB: pulpit 0,5–1 GB + STT 1–2,5 GB; lokalny LLM 3–4B Q4 obok STT; 8B tylko przy STT na CPU. Zarządca rezydencji (`model-residency`): STT+TTS+LLM nie zawsze naraz; wykrycie pełnego ekranu/gry → STT/LLM na CPU lub do chmury.

## 4. Budżet opóźnień i cele

Pomiar: **ostatnia ramka mowy wg VAD → pierwsza próbka na urządzeniu wyjściowym**, mierzone loopbackiem (wirtualny kabel audio na runnerach).

| Etap | Lokalnie | Chmura |
|---|---|---|
| Koniec tury (Smart Turn + cierpliwość) | 250–500 ms | 250–500 ms |
| Finalizacja STT | 150–300 ms | ~150 ms |
| TTFT LLM | 200–600 ms | 200–500 ms |
| TTFB TTS | 200–500 ms | 75–300 ms |
| **Razem** | **~0,8–1,9 s** | **~0,7–1,5 s** |

| Profil | p50 | p95 | Gdzie mierzone | Status |
|---|---|---|---|---|
| C. Jakość-chmura | ≤ 900 ms | ≤ 1500 ms | baseline emulowany | po kluczach, nieblokujące |
| B. Hybrydowy | ≤ 1300 ms | ≤ 2000 ms | baseline emulowany | po kluczach, nieblokujące |
| A. Minimum lokalne | ≤ 2000 ms | ≤ 3000 ms | desktop z emulacją (+ korekta GPU) **i** laptop | go/no-go spike'u (a) w F0; best effort, komunikowane w UI |
| D-AMD16 | jak B | jak B | desktop | mierzone osobno |
| D-CUDA | jak B | jak B | laptop | mierzone osobno |

Uwagi: Bluetooth dodaje +150–300 ms, HFP obniża jakość do 16 kHz → rekomendowany przewód/USB. Mosty CLI (zimny start w sekundach) nigdy nie leżą na ścieżce głosu. Wszystkie cele są **wstępne, do zmierzenia w F0** i zaostrzane po pomiarach.

## 5. Automat dialogu

```
                 ┌──────────────────────────────────────────────────────┐
                 │                                                      │
                 ▼                                                      │
  ┌──────┐  PTT / wake /   ┌───────────┐  VAD: mowa   ┌──────────────┐  │
  │ Idle │ ───────────────▶│ Listening │ ────────────▶│ UserSpeaking │  │
  └──────┘  "zawsze słucham"└───────────┘              └──────┬───────┘  │
      ▲                          ▲                            │ koniec tury │
      │ bezczynność /            │ backchannel /              │ (Smart Turn │
      │ "nie przeszkadzać"       │ "kontynuuj"                ▼  + cierpliwość)
      │                          │                     ┌──────────┐      │
      │                          │                     │ Thinking │      │
      │                          │                     └────┬─────┘      │
      │                          │            1. token / 1. fragment TTS │
      │                          │                          ▼            │
      │                   ┌──────┴──────┐  VAD po AEC  ┌──────────┐      │
      └───────────────────│ Interrupted │◀─────────────│ Speaking │──────┘
        koniec mowy,      └──────┬──────┘ (ducking →   └──────────┘ koniec
        brak wejścia             │        twardy stop)      ▲        odpowiedzi
                                 │ klasyfikacja:            │ "kontynuuj"
                                 │ korekta / uzupełnienie / │ (wznów od cięcia)
                                 │ pytanie / temat / stop   │
                                 ▼                          │
                          ┌──────────────┐                  │
                          │ UserSpeaking │ ─────────────────┘
                          └──────────────┘
```

Przejścia w skrócie: `Idle → Listening → UserSpeaking → Thinking → Speaking → Interrupted → UserSpeaking…`. Stan `Speaking` jest jedynym, w którym samodzielne „nie" liczy się jako przerwanie (§6). Mowa proaktywna wchodzi wyłącznie z `Idle`/`Listening`, nigdy w trakcie `UserSpeaking`, i respektuje „nie przeszkadzać".

## 6. Zatrzymanie dwustopniowe

| Krok | Wyzwalacz | Działanie | Cel czasowy |
|---|---|---|---|
| 1. Ducking | VAD po AEC wykryje mowę w stanie `Speaking` | −15 dB na strumieniu TTS | < 50 ms |
| 2. Twardy stop | ≥ 150–250 ms mowy i klasyfikacja ≠ backchannel | stop TTS, **anulowanie LLM**, czyszczenie kolejki mowy | realnie ≤ ~400 ms od początku wypowiedzi |
| 3. Keyword-spotter | „stop" / „czekaj" w `voice-cmd` | natychmiastowy stop bez LLM | < 300 ms od początku słowa |
| 4. Kill-switch | skrót `Ctrl+Shift+F12` / przycisk w kapsule / zasobnik / „stop" głosem | obsługiwany przez watchdog/broker, poza UI; cisza audio + zabicie Job Objects | < 200 ms |

Zasady:
- „nie" przerywa **tylko** jako samodzielne słowo z pauzą przed i po, wyłącznie w stanie `Speaking` (inaczej fałszywe przerwania przy „nie no, dobrze") — skuteczność mierzona w F2.
- **Dwa zakresy:** *Stop mowy* (Esc / „stop") ≠ *Stop wszystkiego* (kill-switch). `Esc` w UI działa kolejno: zamknij menu/dialog → stop mowy → stop generowania.
- Przerwać można także **pisząc** (composer) i **w trakcie działania narzędzi** (steering, PLAN §9.6) — korekta trafia do agentki między atomowymi krokami.
- Backchannel („mhm", „tak", „aha") nie przerywa; precision backchannelu ≥ 95% (kryterium F2).

## 7. „Usłyszany prefiks"

Po przerwaniu system musi wiedzieć, **co dokładnie usłyszałeś**. Hierarchia źródeł (pierwsze dostępne wygrywa):

| Priorytet | Źródło | Dokładność | Dostępność |
|---|---|---|---|
| 1 | znaczniki słów z TTS (np. ElevenLabs) | słowo | tylko silniki, które je dostarczają |
| 2 | forced alignment na wygenerowanym audio | słowo | lokalnie, koszt CPU |
| 3 | zliczanie odtworzonych próbek skorygowane o opóźnienie urządzenia (`GetStreamLatency`) | granica zdania, flaga `przybliżone` | zawsze |

Metryka: dokładność ±1 słowo; kryterium F2: ≥ 90% przypadków. W UI trybu głosowego prefiks jest podświetlony, a miejsce przerwania oznaczone znacznikiem „przerwano tutaj" (`docs/UI.md`).

## 8. Historia append-only i gałęzie

- Dziennik zdarzeń jest **append-only**; IR trzyma `assistant_full` i `assistant_heard_prefix` osobno.
- **Nie edytujemy wcześniejszych tur** — edycja może unieważnić bloki myślenia Anthropic (bloki są związane z modelem i rozmową). „Edytuj" i „Ponów" w UI tworzą **nowe gałęzie**; każda gałąź to nieedytowana historia, a widoczna rozmowa to projekcja drzewa gałęzi.
- Renderowanie po przerwaniu decyduje adapter dostawcy:
  - Anthropic — pełna tura + dopisana notka „użytkownik usłyszał tylko: „…" i przerwał";
  - dostawcy z natywnym truncate (OpenAI Realtime `conversation.item.truncate`) — obcięcie po stronie dostawcy.
- Spike (g) „append-only vs bloki myślenia Anthropic" jest **odroczony do dodania klucza Anthropic**; ADR (6) obowiązuje tymczasowo.

## 9. Klasy intencji przerwania

Klasyfikator (mały szybki model lub sam LLM) dostaje `{usłyszany_prefiks, nie_powiedziane, wypowiedź}` i zwraca jedną klasę:

| Klasa | Przykład | Reakcja agentki |
|---|---|---|
| korekta | „nie, chodziło mi o wersję z zeszłego tygodnia" | przeplanowanie od punktu cięcia, z poprawką |
| uzupełnienie | „i jeszcze dodaj załącznik" | kontynuacja z rozszerzonym celem |
| pytanie doprecyzowujące | „a to będzie w PDF?" | krótka odpowiedź, potem wznowienie |
| zmiana tematu | „słuchaj, a co z pocztą?" | nowy temat; pytanie „wrócić do tego?" |
| stop / anuluj | „stop", „zostaw to" | zatrzymanie mowy i zadania bieżącego |
| kontynuuj | „mhm, mów dalej" (po pauzie) | wznowienie od punktu cięcia |

Kryterium F2: ≥ 90% na **każdą klasę** (nie średnia), ≥ 50 przykładów na klasę w zamrożonym zestawie.

## 10. Naturalność tur

| Element | Zachowanie |
|---|---|
| Cierpliwość | regulowana; dłużej czeka po hezytacjach („yyy"), krócej po wyraźnym zakończeniu zdania |
| Backchannel agentki | „aha", „rozumiem" w odpowiednich chwilach (nie w środku Twojego zdania) |
| Fillery | maskują opóźnienie, są **poza prefiksem** i przerywalne |
| Mowa proaktywna | zawsze z etykietą (kto i dlaczego), nie w trakcie Twojej mowy, respektuje „nie przeszkadzać" |
| Kolejka mówienia | jedna agentka naraz — `speaker` jako zasób wyłączny w Schedulerze (`scheduler-lite` od F2); przekazanie głosowe („Przekazuję Delcie…") |
| Mówczyni + Myślicielka | szybka agentka w roli Mówczyni (domyślnie Alfa) prowadzi rozmowę i deleguje ciężką pracę; przy długich zadaniach krótko raportuje postępy (gadatliwość w ustawieniach); narracja generowana ze **zdarzeń magistrali**, nie z pamięci modelu; „gotowe" dopiero po weryfikacji Krytyczki (domyślnie Gama) |

## 11. Akustyka i AEC

- **AEC krytyczny:** referencja = własny strumień TTS (najdokładniejsza); loopback jako zapas; porównanie z trybem Communications Windows (uwaga: domyślnie tłumi inne strumienie o 80%). Spike (a) w F0 porównuje trzy warianty.
- Autokalibracja opóźnienia pętli; hot-plug urządzeń; konflikt z aplikacjami w trybie wyłącznym; **słuchawki → agresywniejsze progi barge-in**; adaptacja do szumu otoczenia (próg VAD, głośność); tryb szeptu (cichsza mowa).
- Najtrudniejszy przypadek testowy: **wbudowany mikrofon i głośniki laptopa** (bramka ludzka #6).
- Kryterium F2: fałszywe przerwania ≤ 1/godz. przy 1 h odtwarzania TTS przez głośniki laptopa + tło TV bez Twojej mowy.

## 12. Jakość mówienia

- **Normalizator PL** (liczby, daty, skróty, waluty, URL, kod) + **słownik wymowy** (edytowalny w Ustawieniach → Głos i w Voice Lab; zmiany słownika to pierścień R0 Ulepszacza).
- **Dwa kanały:** *mówiony* (krótki, bez markdown) i *ekranowy* (kod, tabele, linki) — model streszcza głosem, szczegóły idą na ekran.
- Styl przez znaczniki (tempo, energia, emocja) mapowane na możliwości silnika; tagi tylko tam, gdzie silnik je ma.
- Strumieniowanie zdanie-po-zdaniu bez przerw; 24–48 kHz; cache fraz stałych (potwierdzenia, przekazania, komunikaty stanu).
- Żeńskie formy czasowników („zrobiłam", „sprawdziłam") w promptach i mowie.

## 13. Bezpieczeństwo głosu

| Reguła | Szczegóły |
|---|---|
| Pewność STT jest wejściem klasyfikatora ryzyka | niska pewność podnosi klasę ryzyka akcji |
| Weryfikacja właściciela (`voice-speaker`, F5) | wymagana dla akcji ryzykownych; progi zależne od ryzyka; EER ≤ 3% (obce głosy: Common Voice PL + TTS; dla FAR ≤ 0,1% ≥ 3000 prób obcych) |
| Destrukcyjne akcje zlecone głosem | potwierdzenie **nie-głosem na każdym poziomie autonomii, także L4** (odczytane: „Usuwam 14 plików z X — potwierdź", odpowiedź kliknięciem/klawiszem w Broker-UI); wyjątek tylko ręcznie w Broker-UI; reguła Jądra |
| Do czasu `voice-speaker` | każda ryzykowna akcja z głosu potwierdzana w Broker-UI fizycznym wejściem |
| Adresat | komendy z TV / YouTube / rozmowy obok nie wyzwalają akcji: wake-phrase lub PTT + właściciel; dźwięk z otoczenia jest niezaufaną treścią (model zagrożeń, PLAN §8.0) |
| Prywatność | tag sesji decyduje, czy audio może iść do chmurowego STT/TTS/S2S; ekran „co poszło do chmury"; wskaźnik prywatności w zasobniku, gdy mikrofon jest otwarty |
| Red-team | injection dźwiękiem w zestawie ≥ 100 przypadków (F3): 0 eskalacji, 0 egressu bez potwierdzenia |

## 14. Korpus własny (bramka ludzka #3)

Łącznie **~30–45 min Twojej mowy**, nagrywane na desktopie i laptopie, trzymane **lokalnie, poza gitem** (`evals/` tylko hash zamrożonego zestawu testowego). Podział dev/test, test zamrożony (hash) przed implementacją F2. Docelowo ≥ 300 wypowiedzi PL + mieszane PL/EN.

| # | Typ nagrania | Warunki | Czas (orientacyjnie) | Do czego |
|---|---|---|---|---|
| 1 | Swobodna mowa (opowiadanie, pytania do asystenta) | cisza, mikrofon USB/przewód | 5 min | WER bazowy |
| 2 | Swobodna mowa | szum (wentylator, ulica, muzyka cicho w tle) | 4 min | WER w szumie, próg VAD |
| 3 | Swobodna mowa | głośniki laptopa + wbudowany mikrofon | 4 min | AEC, fałszywe przerwania |
| 4 | Swobodna mowa | słuchawki | 3 min | progi barge-in dla słuchawek |
| 5 | Mieszane PL/EN (terminy IT, nazwy produktów, kod czytany) | cisza | 4 min | auto PL/EN, hotwords |
| 6 | Nazwy własne (pliki, foldery, osoby, aplikacje, miasta) | cisza + szum | 3 min | biasing, słownik |
| 7 | Komendy `voice-cmd` („stop", „pauza", „głośniej", „przełącz na Deltę"…) — ≥ 200 prób „stop/anuluj" | różne | 4 min | recall ≥ 99%, reakcja < 300 ms |
| 8 | Przerwania w trakcie odtwarzanego TTS (korekty, uzupełnienia, pytania, zmiana tematu, stop, kontynuuj; ≥ 50 na klasę) | głośniki i słuchawki | 6 min | klasy intencji, prefiks |
| 9 | Backchannel („mhm", „tak", „aha", „nie no, dobrze") podczas mowy agentki | głośniki | 2 min | precision backchannelu, reguła „nie" |
| 10 | Pozytywy słów wywoławczych „Hej Alfa/Beta/Gama/Delta" (≥ 200, różne odległości i głośności) | cisza + szum | 4 min | FRR ≤ 5% (F5) |
| 11 | Enrollment weryfikacji mówcy (czytanie + swobodna mowa, normalnie i szeptem) | cisza | 3 min | `voice-speaker`, tryb szeptu |
| 12 | Dyktowanie z interpunkcją mówioną („przecinek", „nowa linia") | cisza | 3 min | `voice-dictation` (F5) |

Osobno (nie Twoja mowa): ≥ 24 h nagrań tła PL (TV, podcasty) do pomiaru FAR słów wywoławczych ≤ 1/dzień; obce głosy do EER: Common Voice PL + TTS.

## 15. Voice Lab

`voice-lab` powstaje w F0 jako narzędzie pomiarowe (szkielet harnessu razem z `evals/`), a w F2 dostaje UI (Ustawienia → Głos → Voice Lab; ekran makiety nr 16). Testy z fake'ami: fake audio (odtwarzanie WAV + wirtualny zegar), fake STT/TTS, potok testowany deterministycznie.

### 15.1 Co mierzymy i jak

| Metryka | Metoda | Próg (fala) |
|---|---|---|
| Opóźnienie end-to-end p50/p95 | loopback: ostatnia ramka VAD → pierwsza próbka wyjścia; ≥ 100 tur na profil | §4 (F0 spike (a) dla A; F2) |
| Opóźnienia etapów (turn, STT, TTFT, TTFB) | znaczniki czasu na magistrali zdarzeń, strumień „Głos" w logach | budżet §4 |
| WER / CER PL | korpus własny, zamrożony zestaw testowy | WER PL ≤ 12% (F0 (e), F2) |
| Recall „stop/anuluj" i czas reakcji | ≥ 200 prób z korpusu | ≥ 99%, < 300 ms (F2) |
| Precision backchannelu | nagrania typu 9 | ≥ 95% (F2) |
| Fałszywe przerwania | 1 h TTS przez głośniki laptopa + tło TV, bez Twojej mowy | ≤ 1/godz. (F2) |
| Dokładność prefiksu | porównanie z odsłuchem / znacznikami; ±1 słowo | ≥ 90% (F2) |
| Klasyfikacja intencji przerwań | ≥ 50 przykładów na klasę | ≥ 90% na klasę (F2) |
| Echo | poziom resztkowego echa po AEC dla 3 wariantów referencji | spike (a), ADR (11) |
| Odrębność głosów | cos-sim embeddingów ECAPA między parami + ABX | ≤ 0,6; ABX ≥ 90% (F2) |
| Jakość TTS PL | Twoja ślepa ocena 1–5 na 20 zdaniach PL; proxy MOS (UTMOS / TTSDS2) tylko porównawczo (trenowane na EN) | średnia ≥ 4,0 (F0 (e)); no-go = zostają głosy v0 |
| Round-trip STT na TTS | WER/CER transkrypcji własnego TTS | pomocniczo |
| FAR / FRR słów wywoławczych | ≥ 24 h tła PL; ≥ 200 pozytywów | FAR ≤ 1/dzień, FRR ≤ 5% (F5) |
| EER weryfikacji mówcy | Common Voice PL + TTS jako obce | ≤ 3% (F5) |
| Stabilność GPU | 1 h ciągłej pracy whisper.cpp na Vulkan (desktop) i CUDA (laptop) pod limitami baseline | 0 crashy, VRAM w budżecie (F0 (h)) |
| Zasoby | Private Working Set, VRAM, CPU per moduł vs manifest | budżety §3 |

Zasady pomiaru: N ≥ 5 powtórzeń dla elementów niedeterministycznych, przedziały ufności, przypięte wersje modeli; wyniki CPU z desktopu z marginesem +20–30%, wyniki GPU Vulkan × ~2,2 przy ocenie kryteriów baseline; nightly pełna macierz desktop + laptop (zasilanie i bateria).

### 15.2 Tabela kandydatów (spike (e), F0 — do wypełnienia pomiarem)

Kolumny obowiązkowe: kandydat × PL × licencja × streaming × zasoby × TTFB. Wiersze poniżej to lista z planu; wartości „?" uzupełnia spike na desktopie i laptopie, a potem ADR (4) i ADR (11).

**STT**

| Kandydat | PL | Licencja | Streaming | Zasoby (RAM/VRAM, backend) | Opóźnienie finalizacji | WER PL (korpus) | Uwagi |
|---|---|---|---|---|---|---|---|
| whisper.cpp large-v3 | tak | MIT / model OpenAI | partial przez okna | ? (Vulkan/CUDA/CPU) | ? | ? | domyślny D-AMD16; przypięta wersja ≥ 1.8.1 |
| whisper.cpp large-v3-turbo Q5_0 | tak | jw. | jw. | 547 MiB model; ? | ? | ? | domyślny A/B/D-CUDA; halucynuje na szumie → bramka VAD |
| whisper.cpp small Q5 | tak | jw. | jw. | ? (CPU) | ? | ? | fallback |
| Parakeet v3 (ONNX) | tak | ? | ? | ? (CPU) | ? | ? | szybki na CPU; WER PL gorszy niż large-v3 wg źródła wtórnego (FLEURS-PL ~7,3% vs 4,7%) |
| ElevenLabs Scribe v2 RT | tak | chmura | tak | — | ? | ? | profil C |
| Soniox | tak | chmura | tak | — | ? | ? | profil C |
| gpt-4o-transcribe | tak | chmura | ? | — | ? | ? | w benchmarku PUMA (niezweryfikowanym) na równi z large-v3 |
| Qwen3-ASR | tak | chmura | ? | — | ? | ? | tag jurysdykcji |

**TTS**

| Kandydat | PL | Licencja | Streaming | Zasoby | TTFB | Ślepa ocena 1–5 | Klon / voice design | Uwagi |
|---|---|---|---|---|---|---|---|---|
| Pocket TTS + model PL społeczności | tak | CC-BY-4.0 | tak | CPU, RTF ≈ 0,21 (?) | ~200 ms (?) | ? | klon z krótkiej referencji | domyślny lokalny; RTF przy 6 rdzeniach do pomiaru |
| Piper pl_PL | tak | ? | ? | CPU | ? | ? | nie | zapas |
| Chatterbox Multilingual | tak (zgłaszany akcent) | ? | ? | CUDA; RTF ≥ 1 na AMD/ROCm | ? | ? | tak | tylko profil D; poza baseline |
| XTTS-v2 | tak | ? | ? | CUDA | ? | ? | tak | tylko profil D |
| F5-TTS | ? | ? | ? | CUDA | ? | ? | tak | tylko profil D |
| VibeVoice-Realtime | ? | ? | ? | ? | ? | ? | ? | do sprawdzenia |
| ElevenLabs | tak | chmura | tak, znaczniki słów | — | ? | ? | Voice Design + klon | profil B/C; casting |
| Cartesia | tak | chmura | tak | — | ? | ? | ? | profil B/C |
| Google Chirp 3 HD | tak | chmura | ? | — | ? | ? | ? | |
| Azure pl-PL | tak | chmura | tak | — | ? | ? | ? | |
| Gemini TTS | tak | chmura | ? | — | ? | ? | voice design (ToS do sprawdzenia) | zgłaszana regresja akcentu PL w Gemini Live |
| OpenAI TTS | tak | chmura | tak | — | ? | ? | nie | |
| MiniMax Speech 2.8 | tak | chmura | ? | — | ? | ? | klon | kandydat do castingu; tag jurysdykcji |
| Kokoro, Orpheus, Kyutai, Dia, Sesame, Moonshine, Voxtral RT, Aura-2, Qwen3-TTS-RT, GLM-TTS | **brak PL** (wg badania) | — | — | — | — | — | — | poza tabelą, chyba że pomiar wykaże inaczej |

**Pozostałe** (VAD/turn/KWS/mówca): Silero VAD (MIT), TEN VAD, Smart Turn v3.2 (PL; ~8 MB; ~10 ms CPU — do potwierdzenia), openWakeWord (tylko EN → własny trening), WeSpeaker / ECAPA przez sherpa-onnx (CPU). Wszystkie modele wyłącznie w formatach safetensors / ONNX / GGUF z hashami (łańcuch dostaw, PLAN §8.7).

## 16. Ryzyka Vulkan/AMD i fallbacki

| Ryzyko | Objaw | Mitygacja / fallback |
|---|---|---|
| Crashe whisper.cpp na Vulkan (RDNA1/3/4) | `ErrorDeviceLost`, wczesne wyjście `whisper-stream` | przypięta wersja whisper.cpp (min. 1.8.1) + przypięty sterownik Adrenalin; automatyczny fallback na CPU (`-ng`); 1 h testu ciągłego w F0 (h) |
| Halucynacje turbo na szumie | fantomowe transkrypty w ciszy | bramka VAD przed STT; pewność STT do klasyfikatora ryzyka |
| Brak CUDA na baseline i desktopie | Chatterbox / XTTS / F5 z RTF ≥ 1 | na baseline i desktopie: Pocket-PL (CPU) lub chmura; ciężki TTS tylko profil D-CUDA / przyszłe NVIDIA; spike (l) ROCm/PyTorch na RDNA4 opcjonalny |
| HIP/ROCm dla RDNA4 na Windows niestabilny | `ErrorDeviceLost` w części narzędzi | Vulkan jako ścieżka główna na AMD; ROCm tylko po dodatnim spike'u |
| RDNA3 (RX 7600) niepokryty fizycznie | brak pomiaru na docelowej generacji | emulacja limitów + korekta ×2,2; ścieżka CPU jako górna granica; ryzyko zapisane w PLAN §3.5; ewentualna pożyczona karta (PLAN §19 pkt 2) |
| Kwanty IQ na RDNA3/Vulkan | crashe llama.cpp | tylko Q4_K_M i podobne, bez IQ |
| Wyścig o VRAM z grami / pełnym ekranem | spadek FPS, OOM | wykrycie pełnego ekranu → STT/LLM na CPU lub chmura; „nie przeszkadzać" automatyczne |
| Laptop 6 GB VRAM | STT i ciężki TTS nie mieszczą się naraz | zarządca rezydencji wymienia modele; Pocket-PL na CPU; tryb baterii obniża profil |
| GPU OOM w trakcie rozmowy | błąd modułu | stan systemowy w UI, restart modułu przez watchdog, przełączenie na CPU/chmurę bez utraty tury |
| Zmiana urządzenia audio w trakcie | cisza / echo | hot-plug w `voice-audio`, autokalibracja opóźnienia pętli, stan systemowy w UI |
| Polski w STT/TTS słabszy niż zakładano | WER > 12%, ocena TTS < 4,0 | profil chmurowy, korpus własny rozstrzyga; nie zakładamy „nowszy = lepszy"; no-go TTS = głosy v0 (nie blokuje F1) |

## 17. Powiązania

- Biblie głosu, casting i obsady: `docs/PERSONAS.md`.
- Tryb głosowy w UI, pigułka, stany mikrofonu, skróty: `docs/UI.md`.
- Progi akceptacyjne fal: `docs/ACCEPTANCE.md` (zamrażane hashami w `evals/`).
- ADR (4) ML runtime, ADR (6) historia append-only, ADR (11) silniki głosu v0 i audio.
