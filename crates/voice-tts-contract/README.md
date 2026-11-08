# voice-tts-contract

Kontrakt TTS (docs/modules/voice-tts/SPEC.md): trait `Tts` (`synth(TtsRequest, CancelToken) -> TtsStream` —
kanał fragmentów zdanie po zdaniu, `stop`, `warm`, `voices`, `health`, `take_events`). `TtsChunk`: audio 24 kHz
mono, **znaczniki słów** (`WordMark`: indeks słowa w tekście, czas od początku wypowiedzi) i ich rodzaj
(`Native`/`ForcedAlign`/`Estimated`). `VoiceRef`/`VoicePreset` (mówczyni bazowa + wysokość + tempo), `SpeechStyle`.

Wspólne: `split_sentences` (chunker PL ze skrótami: np., m.in., godz., liczebniki porządkowe), `estimate_marks`
(estymata z długości słów i interpunkcji), **głosy v0** `v0_chains()` — 2 mówczynie bazowe Pocket (`pl-f1`,
`pl-f2`) × wysokość/tempo = Alfa 1,00/1,00, Beta 1,06/0,97, Gama 0,92/0,92, Delta 1,12/1,08, zapas Piper
`pl_PL-gosia-medium` z odpowiadającą wysokością — oraz `validate_chains` (żadne brzmienie, także zapasowe, nie
należy do dwóch agentek). Zdarzenia `voice.tts.*` (`started` z TTFB, `chunk`, `finished`, `stopped`, `fallback`,
`cloud.sent`). Nazwy głosów Pocket/Piper i ich licencje — do potwierdzenia castingiem (ADR 0011).
