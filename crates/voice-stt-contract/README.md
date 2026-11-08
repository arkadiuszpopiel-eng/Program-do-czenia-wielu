# voice-stt-contract

Kontrakt STT (docs/modules/voice-stt/SPEC.md): trait `Stt` (async: `configure`, `start_utterance`,
`push` → partial co `partial_every_ms` przy polityce `TwoPass`, `end_utterance` → final, `cancel`, `health`,
`take_events`). `Transcript`: tekst, słowa z czasem i pewnością, język ISO, pewność wypowiedzi (wejście
klasyfikatora ryzyka), opóźnienie, backend. `SttCfg`: silnik (`WhisperCpp{model, backend}`, `Parakeet`,
`Cloud{CloudStt, account, model}` — chmura: tylko typy, adapter w kolejnej fali), język (`auto`/`pl`/`en`),
hotwords (prompt), dwa przebiegi, tag prywatności, bramka VAD `min_speech_ms`.
Wspólne: `UtteranceAudio` (bufor + licznik mowy z detektora energii), `check_privacy` (sesja prywatna + chmura →
`PrivacyBlocked` przed ruchem sieciowym), `hotword_prompt`. Zdarzenia `voice.stt.*` (partial, final,
backend.fallback, model.loaded/unloaded, cloud.sent, gate_rejected).
