# 15 — Głos lokalny wysokiej jakości (dodatek do planu v6)

Stan na **2026-10-11**, wersja 0.0.25. Zgłoszenie właściciela: głos przez API będzie za drogi; są zaawansowane
silniki lokalne — sprawdzić, jak robi to konkurencja, i przedstawić dodatek do planu. Rekomendacja z tego
dokumentu została **zaakceptowana przez właściciela 2026-10-11**.

Ten dokument zmienia kierunek z [ADR 0026](../ADR/0026-glos-z-chmury-jako-opcja.md) (chmura jako droga do
lepszego głosu) na głos lokalny. Zadania są w [roadmapie](03-roadmapa.md), grupa C5.

**Czego ten dokument nie twierdzi.** Niczego nie pobrałam i niczego nie słuchałam. Jakość mowy **po polsku** jest
niezmierzona — żadne znalezione źródło nie porównuje polskiej mowy tych silników. Liczby opóźnień pochodzą
z innych komputerów niż bazowy laptop (RTX 4050 6 GB). Dlatego pierwszym zadaniem jest pomiar i odsłuch (C5.2).

Etykiety pewności jak w [`11-technologie-2026-10.md`](11-technologie-2026-10.md): **P** — przeczytane w źródle
pierwotnym (repozytorium albo karta modelu), **W** — źródło wtórne, **N** — niezmierzone, szacunek.

## 1. Dlaczego nie chmura

Godzina mowy to około 54 tys. znaków (W). Przy 30 minutach mowy agentek dziennie wychodzi około 0,8 mln znaków
miesięcznie.

| Usługa                          | Cena za 1 mln znaków (W) | Miesięcznie przy 0,8 mln znaków |
| ------------------------------- | ------------------------ | ------------------------------- |
| ElevenLabs Multilingual / Flash | ok. 100 / 50 USD         | 40–80 USD                       |
| Cartesia                        | ok. 37 USD               | ok. 30 USD                      |
| OpenAI `gpt-4o-mini-tts`        | ok. 16 USD               | ok. 13 USD                      |
| Google, Amazon Polly            | ok. 4 USD                | ok. 3 USD                       |

Ceny pochodzą z porównań na blogach, które różnią się między sobą — przed jakimkolwiek użyciem trzeba je
sprawdzić w cennikach dostawców. Wniosek jest jednak stabilny: głosy, które brzmią najlepiej, kosztują
kilkadziesiąt dolarów miesięcznie przy codziennej rozmowie, i koszt rośnie z każdą minutą. Głos z chmury
zostaje opcją wyłączoną domyślnie (C1.1a), nie kierunkiem.

## 2. Jak robi to konkurencja

| Program               | Rozpoznawanie mowy   | Synteza mowy                                           | Jak to jest spięte                           | Pewność |
| --------------------- | -------------------- | ------------------------------------------------------ | -------------------------------------------- | ------- |
| Home Assistant Assist | Whisper              | Piper                                                  | każdy silnik to osobna usługa, wymienna      | P       |
| Open WebUI            | lokalny Whisper      | dowolny serwer zgodny z API OpenAI (najczęściej Kokoro) | jeden standardowy interfejs silnika          | P       |
| Kyutai Unmute         | własny, strumieniowy | własny 1,6B — tylko angielski i francuski, 5,3 GB VRAM | kaskada wokół dowolnego modelu, ok. 450 ms   | P / W   |
| NVIDIA ACE (gry)      | —                    | Chatterbox przez ggml, bez Pythona                     | wtyczka C++ z obsługą CUDA, Vulkan i D3D12   | P       |
| Jan                   | brak                 | brak                                                   | —                                            | W       |

Wnioski:

1. **Wszyscy używają kaskady** „mowa → tekst → model → mowa”, z silnikami w osobnych procesach i mową
   wysyłaną kawałkami. Modele „mowa–mowa” w jednym kroku nie obsługują polskiego i nie mieszczą się w 6 GB.
2. **Alfa ma już taką budowę**: kontrakt `TtsBackend`, silniki jako procesy poboczne, mowa zdanie po zdaniu,
   łańcuch głosów z zapasem. Brakuje lepszego silnika i pomiaru — nie architektury.
3. **Chatterbox da się uruchomić bez Pythona.** NVIDIA robi to wtyczką na ggml, a otwarty projekt
   `chatterbox.cpp` (MIT) robi to samo jednym programem. To ta sama rodzina co `llama.cpp` i `whisper.cpp`,
   których Alfa już używa.

## 3. Silniki lokalne z językiem polskim

| Silnik                         | Licencja                                 | Polski         | Sprzęt                                   | Pewność |
| ------------------------------ | ---------------------------------------- | -------------- | ---------------------------------------- | ------- |
| **Chatterbox Multilingual V3** | MIT (kod i wagi)                         | tak, 23 języki | karta graficzna, ok. 3,5 GB VRAM         | P       |
| **Supertonic 3**               | kod MIT, wagi OpenRAIL-M                 | tak, 31 języków | procesor; 99 mln parametrów, ONNX        | P       |
| Piper „Gosia” (obecny)         | stare wydanie MIT; następca GPL-3.0      | tak            | procesor                                 | P       |
| Pocket TTS (Kyutai)            | MIT                                      | **nie** (6 języków) | procesor, ok. 200 ms do pierwszego dźwięku | P / W   |

### Chatterbox Multilingual V3 — kandydat na głos „naturalny”

- Model 500 mln parametrów od Resemble AI; polski jest na liście języków (P).
- **Własny głos z próbki 5–10 sekund** — cztery naprawdę różne głosy agentek zamiast jednego przestrajanego (P).
- Regulacja ekspresji (`exaggeration`, `cfg_weight`) — odpowiada dzisiejszemu ustawieniu „Styl mowy” (P).
- Uruchomienie bez Pythona: `chatterbox.cpp` (MIT) — procesor, CUDA, Vulkan; mowa strumieniowo jako surowy
  dźwięk 24 kHz na wyjście programu; próbka głosu wczytywana przez sam program (P).
- Wagi w formacie GGUF: po kwantyzacji Q4 około 0,3 GB + 0,7 GB zamiast 2,1 GB (P).
- Pomiary autorów `chatterbox.cpp`: wariant wielojęzyczny na Mac Studio M3 Ultra działa 3 razy szybciej niż
  czas rzeczywisty; **na samym procesorze 4 razy wolniej niż czas rzeczywisty** — bez karty się nie nadaje (P).
  Na RTX 4050 nikt tego nie opublikował (N).
- Minusy: brak gotowych wydań dla Windows (budowa ze źródeł), projekt społecznościowy bez numerowanych wydań,
  język polski w nim „podłączony”, ale bez opublikowanych próbek.
- **Znak wodny**: oryginalny Chatterbox dodaje do każdego nagrania niesłyszalny znak (PerTh) (P). Czy robi to
  także `chatterbox.cpp` — nie jest napisane (N). To się wiąże z oznaczaniem treści AI (C2.9).

### Supertonic 3 — kandydat na głos „szybki”

- 31 języków z polskim, bardzo mały, działa na procesorze, przykłady w Rust i C++ (P).
- Ryzyka: repozytorium **zarchiwizowane 2026-09-09** — bez poprawek i wsparcia (P); licencja wag OpenRAIL-M
  ma ograniczenia użycia, których treści jeszcze nie przeczytałam (N); narzędzie do tworzenia własnych głosów
  wyłączono 2026-08-31, zostają głosy wbudowane (P); działa na ONNX Runtime, a ADR 0004 nie pozwala
  dołączać jego binariów do procesu Alfy — trzeba spróbować przez `tract` albo jako proces poboczny.

### Piper — zostaje jako zapas

Szybki i sprawdzony, ale to on jest źródłem skargi „sztywny głos”. Następca jest na GPL-3.0 (potwierdzone
w repozytorium `OHF-Voice/piper1-gpl`), więc nie rozwijamy go dalej — zostaje przypięte wydanie na MIT.

### Odrzucone

| Silnik                                    | Powód                                                    | Pewność |
| ----------------------------------------- | -------------------------------------------------------- | ------- |
| XTTS v2 (Coqui)                           | licencja niekomercyjna; firma zamknięta                  | W       |
| F5-TTS, Fish Speech / OpenAudio, Higgs v3 | wagi tylko do użytku niekomercyjnego                     | W       |
| Kokoro, Orpheus, Dia, Kyutai TTS 1.6B     | brak języka polskiego                                    | W / P   |
| Qwen3-TTS                                 | 10 języków, polskiego nie ma                             | P       |
| Pocket TTS                                | brak polskiego; jest otwarty przepis treningu — badawczo | P / W   |

W kodzie jest wpis silnika „Pocket TTS PL” z głosami `pl-f1` i `pl-f2` (instalacja ręczna). Oficjalny Pocket TTS
polskiego nie ma, więc pochodzenie tego wpisu trzeba wyjaśnić w C5.1 — albo wskazać źródło, albo go usunąć.

## 4. Rekomendacja (zaakceptowana)

Dwa poziomy głosu lokalnego, wybierane automatycznie:

| Poziom        | Silnik                                              | Kiedy                                                                                           |
| ------------- | --------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| **Naturalny** | Chatterbox Multilingual przez `chatterbox.cpp`      | karta graficzna ma wolne ok. 3,5 GB — czyli gdy rozmowę prowadzi model przez API                |
| **Szybki**    | Supertonic 3 po sprawdzeniu; do tego czasu Piper    | kartę zajmuje model lokalny, praca na baterii albo pierwszy dźwięk nie mieści się w budżecie    |

Szacunek pamięci karty na laptopie bazowym (N): rozpoznawanie mowy ok. 1,5 GB + Chatterbox ok. 3,5 GB mieści się
w 6 GB tylko wtedy, gdy model rozmowy nie stoi na karcie. Stąd przełączanie, a nie jeden silnik.

Budżet opóźnienia zostaje ten z `docs/VOICE.md`: pierwszy dźwięk syntezy w 200–500 ms. Jeśli poziom „Naturalny”
go nie spełni na RTX 4050, zostaje jako głos do czytania na głos (gdzie opóźnienie nie przeszkadza), a rozmowę
prowadzi poziom „Szybki”.

## 5. Kryteria wyboru (bramka C5.2)

Silnik wchodzi do programu, jeśli na laptopie bazowym spełnia wszystkie:

| Kryterium                         | Próg                                                                    |
| --------------------------------- | ----------------------------------------------------------------------- |
| Odsłuch właściciela, 20 zdań      | wyraźnie lepszy od Pipera; bez błędów w polskich znakach i liczebnikach |
| Pierwszy dźwięk (p50 / p95)       | poziom rozmowy: do 500 / 900 ms; czytanie na głos: do 1500 ms           |
| Szybkość                          | co najmniej 2 razy szybciej niż czas rzeczywisty                        |
| Pamięć karty                      | do 3,5 GB, zwalniana po bezczynności                                    |
| Licencja silnika, wag i głosów    | pozwala na dołączenie do programu; karta licencji w programie           |
| Stabilność                        | 200 zdań bez awarii procesu i bez urwanego dźwięku                      |

Progi liczbowe są propozycją — po pierwszym pomiarze właściciel je zatwierdza albo zmienia.

Zestaw 20 zdań obejmuje: liczebniki i daty, skróty, nazwy własne, zdania pytające i wykrzyknikowe, zbitki
spółgłoskowe („chrząszcz”, „bezwzględny”), wtrącenia angielskie, jedno długie zdanie złożone.

## 6. Zadania (roadmapa, grupa C5)

| ID   | Zadanie                                                                                                         | Wymaga właściciela                       |
| ---- | --------------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| C5.1 | Licencje i źródła u źródła: pliki LICENSE silników i wag, stan znaku wodnego, wpis „Pocket TTS PL”; `docs/vendor/` | nie                                      |
| C5.2 | Stanowisko pomiarowe i odsłuch: 20 zdań, pierwszy dźwięk, szybkość, pamięć karty                                | **zgoda na pobranie** (ok. 1,5 GB)       |
| C5.3 | Silnik Chatterbox jako proces poboczny za `TtsBackend`                                                          | wynik C5.2                               |
| C5.4 | Głos szybki: próba Supertonic 3 (`tract` albo proces poboczny)                                                  | zgoda na pobranie; licencja wag          |
| C5.5 | Automatyczny wybór poziomu głosu i ustawienie w zakładce „Głos”                                                 | nie                                      |
| C5.6 | Cztery głosy wzorcowe agentek                                                                                   | **skąd próbki**                          |
| C5.7 | Karta licencji każdego silnika i głosu; oznaczenie mowy syntetycznej                                            | decyzja o znaku wodnym                   |

Miejsca w kodzie (bez zmian w kontrakcie potoku głosu):

- `crates/voice-tts-contract/src/types.rs` — wariant `TtsEngine::Chatterbox` już istnieje (sam typ);
- `crates/voice-tts-impl/src/engines.rs` — nowa implementacja `TtsBackend` obok Pipera, proces trwały jak Pocket;
- `crates/app-modules/src/tts.rs` — wykrywanie silnika i kolejność łańcucha;
- `crates/voice-tts-contract/src/voices.rs` — głosy agentek jako próbki wzorcowe;
- `crates/app-models/src/data.rs` — wpisy katalogu (pliki, rozmiary, sumy SHA-256, licencja).

## 7. Decyzje właściciela

| Nr  | Decyzja                                                                                                  | Blokuje          |
| --- | -------------------------------------------------------------------------------------------------------- | ---------------- |
| 1   | Zgoda na pobranie do próby: wagi Chatterbox GGUF (ok. 1 GB) i Supertonic 3 (kilkaset MB)                 | C5.2, C5.4       |
| 2   | Czy niesłyszalny znak wodny w mowie jest akceptowalny (i czy ma być włączony, jeśli da się wybrać)       | C5.7             |
| 3   | Skąd głosy wzorcowe: nagrania z licencją, głosy wbudowane silnika albo własne nagrania                   | C5.6             |
| 4   | Podpis pod zaktualizowanym ADR 0024 (Chatterbox głównym głosem, Piper MIT zapasem)                       | wydanie publiczne |

Głosu prawdziwej osoby bez jej pisemnej zgody nie użyję jako próbki — to reguła, nie decyzja.

## 8. Źródła (sprawdzone 2026-10-11)

- Chatterbox: <https://github.com/resemble-ai/chatterbox>
- `chatterbox.cpp`: <https://github.com/gianni-cor/chatterbox.cpp>
- Wagi GGUF: <https://huggingface.co/hans00/Chatterbox-Multilingual-TTS-GGUF>
- NVIDIA, karta modelu: <https://docs.nvidia.com/ace-for-games/chatterbox-tts/nvigi-chatterbox-model-card.html>
- Supertonic: <https://github.com/supertone-inc/supertonic>, <https://huggingface.co/Supertone/supertonic-3>
- Piper (następca): <https://github.com/OHF-Voice/piper1-gpl>
- Pocket TTS: <https://kyutai.org/tts/>
- Qwen3-TTS: <https://huggingface.co/Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice>
- Home Assistant: <https://www.home-assistant.io/voice_control/voice_remote_local_assistant/>
- Open WebUI: <https://docs.openwebui.com/features/chat-conversations/audio/speech-to-text/stt-config>
- Unmute: <https://www.mintlify.com/kyutai-labs/unmute/architecture/overview>
- Ceny (wtórne): <https://blogs.novita.ai/best-text-to-speech-api-2026/>,
  <https://gradium.ai/content/how-to-compare-tts-pricing-across-providers-2026>
- Przegląd licencji (wtórne): <https://www.yaps.ai/blog/best-local-tts-models>
