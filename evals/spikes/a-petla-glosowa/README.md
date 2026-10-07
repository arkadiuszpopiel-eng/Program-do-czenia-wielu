# Spike (a) — pętla głosowa: mic → VAD → whisper.cpp → Pocket-PL → głośnik, z barge-in

**Cel (PLAN §6.3 profil A, §6.4; VOICE.md §4, §15.1; ACCEPTANCE F0-03, F0-04):** zmierzyć na gotowych binariach,
ile trwa droga od **ostatniej ramki Twojej mowy** do **pierwszej próbki odpowiedzi w głośniku**, i czy przerwanie
(barge-in) podczas mówienia agentki nie wywołuje fałszywych przerwań przez echo własnego TTS.

Ważne: **kod pętli głosowej powstaje dopiero w F2** (`voice-audio`, `voice-vad`, `voice-turn`, `voice-stt`, `voice-tts`,
`voice-dialog`). Ten spike **nie pisze produktu** — składa pętlę z gotowych narzędzi (`whisper-stream`, Pocket TTS z
`h-sprzet/bench-pocket-tts.md`, prosty skrypt sklejający) tylko po to, żeby zmierzyć, czy budżet z §6.4 jest realny
na Twoich maszynach z emulacją baseline. Skrypt sklejający (Python lub PowerShell, kilkadziesiąt linii) tworzy sesja AI
w worktree tego spike'u; tu jest protokół pomiaru.

Kryteria go/no-go (profil A, baseline emulowany + korekta GPU × 2,2): **p50 ≤ 2000 ms, p95 ≤ 3000 ms** z ≥ 50 tur
na desktopie (emulacja) **i** laptopie. AEC: barge-in działa w wariancie „własna referencja" lub loopback bez fałszywych
przerwań przy 30 min odtwarzania TTS przez głośniki.

## Co dokładnie mierzymy (definicja z §6.4)

`opóźnienie = t(pierwsza próbka TTS na urządzeniu wyjściowym) − t(ostatnia ramka mowy wg VAD)`

Etapy składowe (do tabeli, jeśli skrypt sklejający je loguje): koniec tury (VAD + cierpliwość 250–500 ms) →
finalizacja STT (150–300 ms) → „mózg" (w spike'u: **bez LLM** — odpowiedź stała albo echo transkryptu; TTFT LLM
dodajemy z pomiaru `h-sprzet` lub API w F1) → TTFB TTS (200–500 ms).

Metoda pomiaru **niezależna od kodu pętli** (żeby nie mierzyć własnych znaczników czasu, które mogą kłamać):

1. Nagrywamy jednocześnie dwa kanały w Audacity: **L = mikrofon** (to, co słyszy pętla), **P = loopback wyjścia**
   (to, co gra w głośniku), z tego samego zegara (jedno urządzenie wielokanałowe **albo** VB-CABLE jako wspólne urządzenie).
2. Mówisz zdanie, milkniesz; pętla odpowiada. W nagraniu odczytujesz: koniec Twojej mowy na L (ostatnia ramka
   powyżej progu, np. −40 dBFS) i początek odpowiedzi na P. Różnica = opóźnienie tury.
3. Żeby nie czytać 50 tur ręcznie: na początku każdej odpowiedzi TTS skrypt sklejający odtwarza krótki **znacznik**
   (10 ms ton 1 kHz) — wtedy w Audacity: *Analiza → Wykrywanie dźwięku (Sound Finder)* na obu ścieżkach eksportuje
   etykiety z czasami; różnice liczysz w arkuszu. Alternatywa: `ffmpeg -af silencedetect` na każdym kanale osobno.

## Narzędzia

| Narzędzie | Do czego | Skąd |
|---|---|---|
| `whisper-stream.exe` (whisper.cpp) | STT strumieniowe z mikrofonu, wbudowany prosty VAD (`--step 0 --length 30000 -vth 0.6`) | paczka z `h-sprzet/README.md` sekcja B (Vulkan na desktopie, CUDA na laptopie); wymaga SDL2 — jest w paczce lub jako `SDL2.dll` obok |
| Pocket TTS PL | synteza odpowiedzi (CPU) | `h-sprzet/bench-pocket-tts.md` |
| Silero VAD (opcjonalnie, `pip install silero-vad`) | dokładniejszy koniec mowy niż wbudowany VAD `stream` | tylko jeśli skrypt sklejający jest w Pythonie |
| **VB-CABLE** (Virtual Audio Cable, darmowy) | wirtualne urządzenie: wyjście pętli → wejście Audacity (loopback z próbką) | vb-audio.com → VB-CABLE Driver Pack (instalacja jako administrator, restart) |
| **Audacity** | nagranie 2-kanałowe, odczyt czasów, Sound Finder | `winget install Audacity.Audacity` |
| `ffmpeg` | konwersje, `silencedetect` | `winget install Gyan.FFmpeg` |
| Menedżer zadań / `nvidia-smi` | VRAM i CPU w trakcie | — |

Warianty AEC do porównania (ADR (11)), każdy po 30 min odtwarzania TTS przez **głośniki** (nie słuchawki):

| Wariant | Jak w spike'u | Co liczymy |
|---|---|---|
| A. własna referencja | pętla przekazuje strumień TTS jako referencję do AEC (w spike'u: `aec3`/WebRTC APM przez Python `webrtc-audio-processing` albo brak AEC + bramka „nie słuchaj, gdy mówię" jako punkt odniesienia) | fałszywe przerwania / 30 min, resztkowe echo (dB) |
| B. loopback procesowy (Win 10 20348+) | referencja = loopback WASAPI naszego procesu | jw. |
| C. tryb Communications (AEC systemowy) | mikrofon otwarty w trybie komunikacji Windows (Ustawienia → Dźwięk → właściwości mikrofonu → „Ulepszenia dźwięku") | jw. |

W F0 wystarczy: wariant C (bez kodu) + jeden z A/B, jeśli skrypt sklejający zdąży; wynik wpisz do tabeli.

## Przebieg

1. Przygotuj binaria i model (spike (h)), Pocket TTS (venv), VB-CABLE, Audacity.
2. Ustaw urządzenia: pętla słucha **fizycznego mikrofonu**, gra na **fizyczne głośniki**; w Windows
   „Nasłuchuj tego urządzenia" nie włączaj. W Audacity: host WASAPI, wejście = `Głośniki (loopback)` na kanał P
   i mikrofon na kanał L — jeśli Audacity nie pozwoli na dwa różne urządzenia naraz, użyj VB-CABLE: pętla gra na
   `CABLE Input`, Windows „Nasłuchuj" z `CABLE Output` do głośników, Audacity nagrywa `CABLE Output` (P) + mikrofon (L)
   jako dwa osobne nagrania z synchronizacją klaśnięciem na starcie.
3. Uruchom pętlę (skrypt sklejający) **pod emulacją baseline** na desktopie
   (`h-sprzet/emulate-baseline.ps1 -- ...`), zwykle na laptopie.
4. **50 tur**: czytaj po kolei 20 zdań z `e-voice-lab/candidates.md` (2,5 razy), rób 2–3 s przerwy po odpowiedzi.
5. **Barge-in**: 30 min — pętla czyta długi tekst (np. 3 akapity z Wikipedii PL w pętli); Ty **nie mówisz**;
   liczysz, ile razy pętla sama się przerwała (fałszywe przerwania). Potem 20 prób przerwania Twoim głosem —
   liczysz, ile razy przerwała się poprawnie i po ilu ms (ten sam pomiar z nagrania).
6. Odczytaj czasy, policz p50/p95, na desktopie pomnóż część GPU (STT) × 2,2 — jeśli skrypt loguje etapy;
   jeśli nie, pomnóż całość × 1,5 jako ostrożne przybliżenie i zaznacz to.

## Szablon wyników — `results/<maszyna>-<data>.md`

```markdown
# Spike (a) — <maszyna> — <RRRR-MM-DD>

| Pole | Wartość |
|---|---|
| Maszyna / emulacja | desktop-emu (affinity 0xFFF, 12 GB) / laptop |
| STT | whisper-stream <tag>, backend, model, parametry (--step, --length, -vth) |
| VAD / koniec tury | wbudowany stream / Silero + cierpliwość <ms> |
| TTS | Pocket TTS PL <wersja>, model, referencja |
| Audio | mikrofon, głośniki, tryb (Communications tak/nie), VB-CABLE tak/nie |
| Metoda odczytu | Audacity Sound Finder / silencedetect / ręcznie |

## Opóźnienie tury (n = 50)

| Metryka | Zmierzone [ms] | Po korekcie (desktop: GPU ×2,2) [ms] | Cel profilu A |
|---|---|---|---|
| p50 | | | ≤ 2000 |
| p95 | | | ≤ 3000 |
| min / max | | | — |
| etapy (jeśli logowane): koniec tury / STT / TTS TTFB | | | 250–500 / 150–300 / 200–500 |

## Barge-in i AEC

| Wariant AEC | Fałszywe przerwania / 30 min | Poprawne przerwania (z 20) | Czas reakcji na przerwanie p50 [ms] | Echo resztkowe (opis / dB) |
|---|---|---|---|---|
| C. Communications | | | | |
| A. własna referencja / B. loopback | | | | |

## Zasoby w trakcie (Menedżer zadań)
CPU %, RAM pętli, VRAM szczyt:

## Uwagi
Halucynacje STT na ciszy? Crashe Vulkan/CUDA? Ucięte początki odpowiedzi? Co bolało?
```

**Co skopiować z powrotem:** ten plik `.md`, plik etykiet z Audacity (`*.txt`, mały) lub arkusz z 50 czasami (`.csv`),
i 2–3 przykładowe nagrania tur **do `evals/corpus/spike-a/`** (poza gitem; w wynikach tylko nazwy plików).
