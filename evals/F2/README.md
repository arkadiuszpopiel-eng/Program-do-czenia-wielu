# evals/F2 — zestaw dev/test głosu (ACCEPTANCE F2)

Zestaw do kryteriów F2-03 … F2-08 z `docs/ACCEPTANCE.md` §5: WER PL, recall „stop/anuluj” z reakcją
< 300 ms, precision backchannelu, fałszywe przerwania na godzinę, dokładność usłyszanego prefiksu (±1 słowo),
klasy intencji przerwań. Narzędzie: `alfa-voice-eval` (crate `voice-pipeline-impl`, moduł `eval`).

**Nagrania i transkrypcje nie trafiają do gita** (dane biometryczne, VOICE.md §13): korpus leży w
`evals/corpus/` (`.gitignore`), do repo trafia tylko lista zamrożenia (same skróty SHA-256 i ścieżki).

## Co jest w repo

| Plik | Zawartość |
|---|---|
| `manifest.schema.json` | JSON Schema jednej linii manifestu (generowany z kodu; test snapshotu) |
| `results.schema.json` | JSON Schema jednej linii wyników |
| `samples/manifest.ndjson` | 3 próbki syntetyczne (swobodna mowa, „stop”, „mhm”) — CI sprawdza na nich format i runner |

## 1. Nagrywanie (bramka ludzka #3)

Nagrywaj według `evals/spikes/e-voice-lab/protokol-nagran.md` (12 typów, sprzęt, Audacity, nazwy plików,
konwersja do 16 kHz). Do zestawu F2 potrzebne są typy 1–9 (10–12 to F5). Minimalne liczności z ACCEPTANCE:
≥ 200 prób „stop/anuluj” w trakcie mowy agentki, ≥ 100 backchanneli i ≥ 100 prawdziwych przerwań,
≥ 50 przerwań na każdą klasę intencji, ≥ 50 fraz „nie no, dobrze”; F2-06 to osobna godzina TTS przez
głośniki laptopa z tłem TV, **bez** Twojej mowy (dziennik sesji, niżej).

Wskazówki do typów 7–9:
- **Komendy (7):** jedna komenda = jedna pozycja; w długim pliku zaznacz fragment (`segment`) i początek słowa
  komendy (`onset_ms`, z Audacity: zaznacz początek fali słowa, odczytaj czas w ms od początku pliku).
- **Przerwania (8) i backchannel (9):** nagrywaj na tle odtwarzanego TTS (głośniki lub słuchawki). Jeżeli znasz
  tekst i moment startu odtwarzanej wypowiedzi, wpisz `tts` (tekst, długość audio, ile ms było już odtworzone na
  początku pozycji) — runner policzy wtedy usłyszany prefiks tym samym automatem co potok. `heard_words` to
  prawdziwa liczba usłyszanych słów (z odsłuchu: ile słów agentki padło przed Twoim wejściem w słowo).
- Klasę intencji zapisuj od razu (protokół: powiedz numer klasy przed próbą albo notuj w `INDEX.md`).

## 2. Manifest (NDJSON)

Jeden plik `evals/corpus/f2/manifest.ndjson` (poza gitem), **jedna pozycja na linię**, puste linie i `#…`
pomijane. Ścieżki `audio` są względne wobec katalogu korpusu i wskazują pliki **WAV 16 kHz mono** (`16k/…`).

```json
{"id":"d-2026-10-05-t07-01-003","audio":"16k/desktop/2026-10-05/t07-komendy-cisza-usb-01.wav","kind":"command","split":"dev","conditions":{"machine":"desktop","environment":"quiet","mic":"usb"},"transcript":"stop","command":"stop","segment":{"start_ms":12400,"end_ms":13300},"onset_ms":12650}
{"id":"l-2026-10-06-t08-02-011","audio":"16k/laptop/2026-10-06/t08-przerwania-glosniki-wbud-02.wav","kind":"interruption","split":"test","conditions":{"machine":"laptop","environment":"speakers","mic":"builtin"},"transcript":"nie, chodziło mi o czwartek","intent":"correction","heard_words":7,"segment":{"start_ms":40100,"end_ms":42900},"tts":{"text":"W środę masz trzy spotkania, pierwsze o dziewiątej…","audio_ms":9000,"played_at_start_ms":2300}}
```

| Pole | Wymagane | Opis |
|---|---|---|
| `id` | tak | unikalny, `[a-z0-9._-]` |
| `audio` | tak | ścieżka względna `.wav` (bez `..`) |
| `kind` | tak | `free_speech` (typy 1–4), `mixed_pl_en` (5), `names` (6), `command` (7), `interruption` (8), `backchannel` (9), `wake` (10), `enroll` (11), `dictation` (12) |
| `split` | tak | `dev` / `test` — protokół: nieparzysty `<nr>` pliku → dev, parzysty → test; **jeden plik tylko w jednym podziale** |
| `conditions` | tak | `machine` (`desktop`/`laptop`/`synthetic`), `environment` (`quiet`/`noise`/`speakers`/`headphones`), `mic` (`usb`/`builtin`/`headset`/`virtual`) |
| `transcript` | WER, `command`, `backchannel` | dokładnie to, co powiedziano; liczby słownie, wtrącenia EN w oryginale |
| `command` | `command` | `stop`, `cancel`, `wait`, `pause`, `resume`, `repeat`, `volume_up`, `volume_down`, `mute_mic`, `switch_persona`, … |
| `onset_ms` | zalecane dla `command` | początek słowa komendy w pliku — punkt odniesienia reakcji |
| `intent` | `interruption` | `correction`, `addition`, `clarify`, `topic_change`, `stop_cancel`, `continue` |
| `heard_words` | zalecane dla `interruption` | prawdziwa liczba usłyszanych słów (F2-07) |
| `tts` | opcjonalne (8–9) | `{ text, audio_ms, played_at_start_ms }` |
| `segment` | opcjonalne | `{ start_ms, end_ms }` — fragment dłuższego pliku |
| `synth` | tylko `synthetic` | przepis próbki syntetycznej (CI) |
| `note` | opcjonalne | uwagi (co szumiało, odległość) |

Wyniki (`results.ndjson`, też poza gitem): linia na pozycję — `id`, `hypothesis` (transkrypt końcowy),
`command`, `reaction_ms`, `interrupted`, `intent`, `heard_words`. Produkuje je `alfa-voice-eval run`
albo Voice Lab na żywym potoku (ten sam format).

## 3. Zamrożenie podziału test

Przed strojeniem F2 (protokół: test zamrożony przed implementacją) — **na maszynie z korpusem**:

```powershell
cargo run -p voice-pipeline-impl --bin alfa-voice-eval -- check  evals/corpus/f2/manifest.ndjson --audio-root evals/corpus
cargo run -p voice-pipeline-impl --bin alfa-voice-eval -- freeze evals/corpus/f2/manifest.ndjson --audio-root evals/corpus > evals/acceptance/F2/test.sha256
cd evals/acceptance; find F* -type f | LC_ALL=C sort | xargs sha256sum > HASHES   # README w evals/acceptance
```

`test.sha256` ma format `sha256sum`: skrót kanonicznych pozycji test (`manifest:test`) i skrót każdego pliku
audio test — bez transkrypcji i audio, więc może trafić do gita. Commit `test.sha256` + `HASHES` wymaga
przeglądu człowieka (AGENTS.md). Każda ocena podziału test zaczyna się od `verify` (lub `ALFA_F2_FROZEN`):
zmiana pliku, transkrypcji, etykiety albo liczby pozycji = błąd.

## 4. Przebieg na prawdziwych modelach (maszyna użytkownika)

Potrzebne: `whisper-cli` z whisper.cpp (przypięta wersja ≥ 1.8.1, Vulkan/CUDA/CPU) i model GGML
(`ggml-large-v3-turbo-q5_0.bin` dla profilu A/B).

```powershell
$m = "evals/corpus/f2/manifest.ndjson"
cargo run --release -p voice-pipeline-impl --bin alfa-voice-eval -- verify $m --audio-root evals/corpus --frozen evals/acceptance/F2/test.sha256
cargo run --release -p voice-pipeline-impl --bin alfa-voice-eval -- run $m --audio-root evals/corpus --split test `
    --whisper-cli C:\tools\whisper\whisper-cli.exe --model C:\models\ggml-large-v3-turbo-q5_0.bin --out evals/corpus/f2/results-test.ndjson
cargo run --release -p voice-pipeline-impl --bin alfa-voice-eval -- score $m evals/corpus/f2/results-test.ndjson --split test --session-log evals/corpus/f2/f2-06-session.ndjson
```

- `run --mode prefix` (domyślnie): partial = model na audio „do teraz” co 100 ms w trakcie mowy (jak
  keyword-spotter potoku), final = cała pozycja; prefiksy zapisywane chwilowo w `--work`
  (domyślnie `<audio-root>/.work`, poza gitem) i usuwane od razu. `--mode timeline`: jedna transkrypcja
  ze znacznikami słów na pozycję — szybciej, ale partial widzi słowo dopiero po jego końcu (pesymistyczne
  dla krótkich backchanneli). Reakcja to czas akustyczny (kadencja partiali + długość słowa); opóźnienie
  obliczeń na żywym sprzęcie mierzy Voice Lab.
- Komendy: `GrammarRecognizer` z `Grammar::default_pl_en()`; dialog: `default_machine()` — te same rdzenie co
  w potoku. VAD: detektor energii (pozycje są już wycięte); Silero i AEC mierzy Voice Lab na żywo.
- `score` wypisuje tabelę Markdown z progami; kod wyjścia 1, gdy któreś kryterium nie spełnia progu.
- F2-06 (fałszywe przerwania/h): dziennik sesji = NDJSON zdarzeń magistrali (`core-bus` `Event`) z godziny
  odtwarzania TTS przez głośniki laptopa z tłem TV, bez mowy właściciela; liczone są `voice.dialog.interrupted`
  na godzinę. Dziennik eksportuje Voice Lab / strumień „Głos” w logach.
- To samo jako test: `cargo test -p voice-pipeline-impl --test f2_eval -- --ignored` ze zmiennymi
  `ALFA_F2_MANIFEST`, `ALFA_F2_AUDIO_ROOT`, `ALFA_WHISPER_CLI`, `ALFA_WHISPER_MODEL`
  (opcjonalnie `ALFA_F2_SPLIT=dev|test`, `ALFA_F2_FROZEN`, `ALFA_F2_MODE=timeline`).

Wyniki (tabela z `score`, bez transkrypcji) można skopiować do `evals/spikes/e-voice-lab/results/` lub do
artefaktu CI self-hosted. Progi są w kodzie tylko do odczytu (`eval::report`); zmiana progów = zmiana
`docs/ACCEPTANCE.md` z zatwierdzeniem człowieka.

## 5. CI (bez korpusu)

`cargo test -p voice-pipeline-impl --test f2_eval`: snapshot schematów, walidacja `samples/manifest.ndjson`,
3 próbki syntetyczne przez runner offline (oś słów zamiast modelu) i CLI (`check`, `synth`, `freeze`,
`verify`, `score`). Aktualizacja schematów: `UPDATE_SCHEMAS=1 cargo test -p voice-pipeline-impl --test f2_eval`.
Próbki syntetyczne lokalnie: `alfa-voice-eval synth evals/F2/samples/manifest.ndjson <katalog>`.
