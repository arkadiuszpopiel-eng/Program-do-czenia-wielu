# voice-pipeline-fake

Atrapa `voice-pipeline` (docs/modules/voice-pipeline/SPEC.md §Fake) — **tylko jako `dev-dependency`** testów UI,
`app-core` i `agent-runtime`. Bez audio i bez modeli:
- `FakePipeline` implementuje `VoicePipeline`; czas płynie krokami (`tick_ms`);
- wejścia UI (`Toggle`, `Ptt`, `SetMuted`, `SwitchPersona`, `Typed`, `StopSpeech`, `Deactivate`) zmieniają stan mikrofonu
  (jeden naraz), fazę i agentkę jak w potoku; wszystkie wejścia są zapisywane (`inputs()`);
- skrypt na osi czasu (`script(at_ms, ScriptStep)`): faza, kto mówi, transkrypt częściowy, usłyszany prefiks, opóźnienia
  tury, poziom, dowolne `PipelineEvent` → zdarzenia `voice.pipeline.*` (`events()`).

Przechodzi wspólny test kontraktowy (`contract_tests::run_all`).
