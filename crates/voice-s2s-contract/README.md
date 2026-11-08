# voice-s2s-contract

Kontrakt trybu speech-to-speech w chmurze (docs/modules/voice-s2s/SPEC.md): `S2sClient::connect` (walidacja
`S2sCfg` — sesja `Private` → `PrivacyBlocked`, preset głosu dostawcy dla agentki, 16/24 kHz, limit 1–120 min) i
`S2sSession` (audio mikrofonu, zatwierdzenie tury, `cancel_response`, natywne `truncate` do usłyszanego miejsca —
`truncate_point`, zdarzenia `S2sEvent`, zamknięcie). `S2sProvider::interruption()` → OpenAI Realtime =
`InterruptionRendering::NativeTruncate`, Gemini Live = `AppendNote` (`truncate` → `Unsupported`). Zdarzenia magistrali
`voice.s2s.*` bez audio i treści; `audio_sent` zasila ekran „co poszło do chmury”. Testy kontraktowe: feature
`contract-tests`. Adapter chmurowy (`voice-s2s-impl`) — później.
