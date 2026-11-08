# voice-pipeline-impl

Implementacja modułu `voice-pipeline` (docs/modules/voice-pipeline/SPEC.md, VOICE.md §18): runtime rozmowy
głosowej składający kontrakty `voice-*` — zależy **wyłącznie od kontraktów**; konkretne moduły podaje korzeń
kompozycji (`app-*`) w `PipelineParts`.

- `Pipeline` (`VoicePipeline`): krok wątku przetwarzania co ~10 ms (`step`) i pętla `run(CancellationToken)`.
  Mikrofon → DSP (AEC z referencją wyjścia) → `EchoGate` → VAD → STT (partial/final, partial na żądanie co
  100 ms jako keyword-spotter w trakcie mowy agentki) → `voice-cmd` / `voice-turn` → automat `voice-dialog` →
  `ReplySource` → `voice-persona` → `voice-tts` → `voice-audio`. Wolne operacje są przyszłościami odpytywanymi
  w kolejnych krokach (bez spawnowania); wątek RT urządzenia nie alokuje.
- `SchedSpeakerLock` — głośnik jako dzierżawa `scheduler-lite`; mikrofon przez `MicArbiter` (`voice-wake-contract`).
- `EchoGate` — reguła echa obok AEC (detektor podwójnej mowy na poziomach, uczone sprzężenie).
- `ProviderReply` — `ReplySource` na `ModelProvider` z historią append-only (tura przerwana renderowana przez
  `render_interrupted_turn` z usłyszanym prefiksem; `raw_prefix` mapuje słowa mówione na oryginał).
- `eval` + `alfa-voice-eval` — zestaw F2 (evals/F2/README.md): manifest NDJSON, walidacja, zamrożenie podziału
  test (SHA-256), runner offline (`whisper-cli` w trybie `prefix`/`timeline`, `GrammarRecognizer`,
  `default_machine()`), metryki i raport z progami ACCEPTANCE F2.

## Testy

`cargo test -p voice-pipeline-impl` — wszystko na atrapach i wirtualnym zegarze (bez `sleep`):
`e2e_turn` (pełna tura, TTFA profilu A p50/p95 na 100 scenariuszach, awaria STT GPU → CPU, rezydencja modeli,
zdarzenia bez audio, test kontraktowy), `e2e_barge` (barge-in z echem i bez, backchannel co 3 s przez 60 s,
„stop”/„czekaj”, echo TTS w „pokoju”), `e2e_persona_ptt` (zmiana agentki 20/20, PTT), `props` (property:
głośnik, brak audio po `StopTts`, mikrofon w jednym stanie), `f2_eval` (format zestawu F2, próbki syntetyczne, CLI;
`--ignored` — prawdziwe modele i korpus), `module`. Pomiary wypisują się w `--nocapture`.
