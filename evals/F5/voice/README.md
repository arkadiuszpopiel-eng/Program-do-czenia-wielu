# evals/F5/voice — zestawy głosu rozszerzonego (F5-05…08, F5-10, F5-11)

Kryteria z `docs/ACCEPTANCE.md` §8. W repo są **formaty, schematy i próbki syntetyczne CI**; prawdziwy korpus
(nagrania właściciela, tło PL, Common Voice PL) leży **poza gitem** w `evals/corpus/f5/` na maszynie z runnerem
self-hosted (bramka ludzka #3). **Status: propozycja autora modułów — do zamrożenia hashem po akceptacji
właściciela/modelu-recenzenta** (ACCEPTANCE §1); hashe plików z repo: `MANIFEST.json` (status `proposed`), hashe
korpusu (podział `test`): `alfa-wake-eval freeze`.

| Plik | Kryterium | Test CI |
|---|---|---|
| `samples/wake.ndjson` + `wake-manifest.schema.json` | F5-05 FAR ≤ 1/dzień, F5-06 FRR ≤ 5 % (format §1) | `crates/voice-wake-impl/tests/eval.rs` |
| `samples/speaker.ndjson` + `speaker-manifest.schema.json` | F5-07 EER ≤ 3 %, F5-08 FAR ≤ 0,1 % przy progu ścisłym (format §2) | `crates/voice-speaker-impl/tests/eval.rs` |
| `dictation-cases.json` | F5-10 — część CI: normalizacja odwrotna (format §3) | `crates/voice-dictation-contract/src/tests.rs` |
| — | F5-11 — czytanie zaznaczenia w 5 aplikacjach (§4) | kontraktowe `voice-readaloud-*` na `platform-fake` |

Próbki syntetyczne sprawdzają **mechanikę** runnerów (parsowanie, liczenie FAR/FRR/EER, przegląd progów), nie jakość
modeli — progi akceptacyjne mierzy się wyłącznie na korpusie.

## §1. Słowa wywoławcze (`alfa-wake-eval`)

NDJSON, jedna pozycja na linię (puste i `#…` pomijane); schemat: `wake-manifest.schema.json`
(`cargo run -p voice-wake-impl --bin alfa-wake-eval -- schema`).

| Pole | Opis |
|---|---|
| `id` | unikalny `[a-z0-9._-]` |
| `kind` | `wake_positive` (fraza właściciela — FRR) / `wake_background` (tło bez frazy — FAR) |
| `split` | `dev` (strojenie progu, histerezy) / `test` (ocena, zamrażany) |
| `persona`, `phrase` | pozytyw: adresatka (`alfa`, `delta`, imię z Kreatora) i wypowiedziana fraza |
| `segment` | pozytyw: `{start_ms, end_ms}` frazy — wykrycie musi paść w oknie (do 500 ms po końcu) i dla właściwej agentki |
| `conditions` | `machine` (`desktop`/`laptop`/`synthetic`), `environment` (`quiet`/`noise`/`speakers`/`tv`/`podcast`/`conversation`), `mic`, `distance_m` |
| `audio` | WAV 16 kHz mono PCM16, ścieżka względna wobec `--audio-root` (bez `..`, `\`, `:`) |
| `synth` | zamiast `audio` (CI): `{kind: tone|speech, secs, lead_secs, tone_hz, seed}` |

Wymagania korpusu: ≥ 24 h tła PL (TV, podcasty, rozmowy) i ≥ 200 pozytywów właściciela w podziale `test`
(inaczej `sufficient = false`). Wynik `run`: JSON z `at_threshold` (FAR/dzień, FRR, pomyłki agentki), przeglądem progów
0,30–0,95, rekomendacją (najniższe FRR przy FAR ≤ 1/dzień) i flagami `f5_05_far_ok`, `f5_06_frr_ok`; `--out` zapisuje
wynik per pozycja (bez audio). Kod wyjścia ≠ 0, gdy kryterium niespełnione.

```
cargo run -p voice-wake-impl --bin alfa-wake-eval -- check evals/corpus/f5/wake.ndjson --audio-root evals/corpus
cargo run --release -p voice-wake-impl --bin alfa-wake-eval -- run evals/corpus/f5/wake.ndjson \
    --audio-root evals/corpus --model modele/hej.kws.json --split test --out wyniki-wake.ndjson
cargo run -p voice-wake-impl --bin alfa-wake-eval -- freeze evals/corpus/f5/wake.ndjson --audio-root evals/corpus
```

## §2. Weryfikacja mówcy (`alfa-speaker-eval`)

NDJSON; schemat `speaker-manifest.schema.json`.

| Pole | Opis |
|---|---|
| `id` | unikalny `[a-z0-9._:-]` |
| `speaker` | `owner` albo identyfikator obcego mówcy (`cv:<hash>`, `tts:<głos>`) |
| `role` | `enroll` (rejestracja właściciela, typ 11 protokołu nagrań; ≥ 3) / `trial` (próba weryfikacji) |
| `split` | `dev` / `test` |
| `source` | `own` / `common_voice` / `tts` (także próby podszycia) / `synthetic` |
| `audio` / `synth` | WAV 16 kHz mono albo (CI) `{f0, seed, secs}` |
| `note` | warunki (szept, odległość) |

Runner rejestruje właściciela w **profilu tymczasowym w pamięci** (nigdy w profilu użytkownika), liczy kosinus każdej
próby i raport: EER i próg EER, próg dla FAR ≤ 0,1 % (`strict`), FAR/FRR przy progach z konfiguracji, flagi
`f5_07_ok`, `f5_08_ok`, `sufficient_impostors` (≥ 3000 prób obcych). `--out` zapisuje wyniki prób (bez audio
i embeddingów).

```
cargo run --release -p voice-speaker-impl --bin alfa-speaker-eval -- run evals/corpus/f5/speaker.ndjson \
    --audio-root evals/corpus --model modele/wespeaker.speaker.json --split test --out próby.ndjson
```

Wyznaczone progi wpisuje się do `[voice.speaker] threshold_standard` (≈ próg EER) i `threshold_strict` (FAR ≤ 0,1 %).

## §3. Dyktowanie — normalizacja (`dictation-cases.json`)

`{version, criterion, status, threshold, cases: [{id, input, expected}]}` — `input` to final STT, `expected` tekst do
wpisania po normalizacji odwrotnej (komendy interpunkcji PL, liczby tylko jednoznaczne, „dosłownie”, kontekst
wielkiej litery). CI: 30/30. Pomiar F5-10 na sprzęcie (self-hosted, `platform-windows-gui-impl`): te same wypowiedzi
nagrane/odtworzone do Notatnika, WordPada, przeglądarki, VS Code i Worda; zgodność = odległość edycyjna znaków
wpisanych vs `expected` — próg ≥ 95 %.

## §4. Czytanie na głos (F5-11)

Bez osobnego pliku danych: na runnerze self-hosted w każdej z 5 aplikacji zaznacza się akapit wzorcowy
(`docs/modules/voice-readaloud/SPEC.md`), `ReadScope::Selection` musi zwrócić ten sam tekst (UIA albo zapas Ctrl+C
z przywróconym schowkiem) i odczytać wszystkie zdania. CI pokrywa to testami kontraktowymi na `platform-fake`.

## Prywatność korpusu

Nagrania właściciela i embeddingi nie trafiają do gita, logów ani wyników (`--out` zawiera tylko identyfikatory,
wyniki i decyzje). Common Voice PL — CC0; głosy TTS zgodnie z licencją modelu.
