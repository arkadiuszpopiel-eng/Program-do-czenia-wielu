# voice-pipeline-contract

Kontrakt potoku głosu (docs/modules/voice-pipeline/SPEC.md, PLAN §6.2–6.5, VOICE.md „Implementacja”):
runtime, który składa kontrakty `voice-*` w rozmowę i wykonuje polecenia automatu dialogu.

- `VoicePipeline` — `input(PipelineInput)` (PTT/przełącznik, composer, `Esc`, wyjście z trybu głosowego,
  wyciszenie, DND, zmiana agentki, mowa proaktywna), `step()` (jeden krok wątku przetwarzania, bez blokowania),
  `status()` (`PipelineStatus`: faza, stan mikrofonu — jeden naraz, kto mówi, poziom, transkrypt częściowy,
  usłyszany prefiks, opóźnienia tury `TurnLatency`).
- Port `ReplySource` — `start(ReplyRequest, CancellationToken) -> ReplyStream` (strumień tekstu odpowiedzi) i
  `finish(turn, ReplyOutcome)` (historia append-only: tura trafia do historii raz, pełna, z usłyszanym prefiksem).
- `PipelineCfg` — krok 10 ms, partial barge-in co 100 ms (ścieżka keyword-spottera), pre-roll 200 ms, reguła
  echa `EchoGateCfg` (okno 300 ms, sprzężenie startowe −6 dB, margines 6 dB), ducking ≤ 50 ms, pigułka co 100 ms,
  wyprzedzenie syntezy `speak_ahead_ms` 8000 (kontrola przepływu: TTS nie zapełnia kolejki miksera szybciej, niż
  gra wyjście), maks. wypowiedź 30 s, `privacy` (sesja prywatna → STT/LLM tylko lokalnie), teksty fillera/awarii.
- Zdarzenia `voice.pipeline.*` (`PipelineEvent`): `pill`, `transcript`, `heard_prefix`, `persona_switched`,
  `latency`, `volume`, `kill_switch_requested`, `cancel_task`, `degraded` — bez treści audio.
- Testy kontraktowe (feature `contract-tests`): przełącznik mikrofonu, wyciszenie, zmiana agentki w locie.
