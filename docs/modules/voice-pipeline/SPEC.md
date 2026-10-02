# voice-pipeline — SPEC (v1: kontrakt, fake i impl w repo; F2)

## Cel
Runtime rozmowy głosowej: składa kontrakty `voice-*` w potok mikrofon → DSP (AEC z referencją wyjścia TTS) → reguła echa → VAD → STT (partial/final) → komendy szybkie (`voice-cmd`) i koniec tury (`voice-turn`) → automat `voice-dialog` → odpowiedź (port `ReplySource`) → `voice-persona` (normalizacja PL, chunker, styl) → `voice-tts` (głos agentki) → `voice-audio` (wyjście z licznikiem próbek). Wykonuje polecenia automatu (ducking, twardy stop, głośnik, fillery, wznowienia), pilnuje wyłączności głośnika/mikrofonu (`scheduler-lite`), adresowania i PTT (`voice-wake`) oraz rezydencji modeli (`model-residency`). PLAN §6.2–6.5, VOICE.md „Implementacja”.

## Fala i priorytet
F2, P0 (warunek kryteriów F2-01, F2-04…F2-07, F2-11, F2-12, F2-14).

## Kontrakt (v1, `crates/voice-pipeline-contract`)
```rust
#[async_trait] pub trait VoicePipeline: Send { fn input(&mut self, PipelineInput); async fn step(&mut self) -> StepReport; fn status(&self) -> PipelineStatus; }
pub enum PipelineInput { Ptt { pressed }, Toggle, Typed { text }, StopSpeech /* Esc */, Deactivate, SetMuted { muted }, SetDoNotDisturb { on }, SwitchPersona { persona }, Proactive { persona, text, reason } }
pub struct PipelineStatus { now_ms, phase, mic: MicState /* jeden naraz */, mic_open, speaker: Speaker /* Nobody|User|Agent(persona) */, persona, level_db, partial, heard_prefix, turns, interruptions, echo_gated_frames, latency: TurnLatency }
pub trait ReplySource: Send + Sync { fn start(&self, ReplyRequest, CancellationToken) -> ReplyStream /* ReplyChunk */; fn finish(&self, turn, ReplyOutcome); }
pub enum ReplyChunk { Text(String), Done, Failed(String) }   pub enum ReplyOutcome { Completed, Interrupted { heard, approximate } }
pub enum PipelineEvent { Pill, Transcript, HeardPrefix, PersonaSwitched, Latency, Volume, KillSwitchRequested, CancelTask, Degraded }   // voice.pipeline.*
```
`PipelineCfg`: krok 10 ms, partial barge-in 100 ms, pre-roll 200 ms, maks. wypowiedź 30 s, `EchoGateCfg` (ogon 300 ms, sprzężenie startowe −6 dB, margines 6 dB, próg referencji −60 dBFS, min. pewność AEC 0,5), `speak_ahead_ms 8000`, ducking 20 ms / powrót 50 ms, pigułka co 100 ms, persona domyślna `alfa`, `privacy`, chunker, teksty fillera i awarii.
Impl: `Pipeline::new(PipelineParts, PipelineCfg)` + `run(CancellationToken)` (pętla `tokio::time::interval`), `SchedSpeakerLock` (głośnik na `scheduler-lite`), `EchoGate`, `ProviderReply` (`ModelProvider` + historia append-only: tura użytkownika; po przerwaniu `render_interrupted_turn` z usłyszanym prefiksem), `raw_prefix` (słowa usłyszane po normalizacji → oryginał odpowiedzi), `eval` (zestaw F2) i CLI `alfa-voice-eval`.

## Wątki, kolejki, anulowanie
- Wątek RT urządzenia (`voice-audio`) — bez alokacji; kolejki SPSC: przechwytywanie, mikser, referencja AEC.
- Wątek przetwarzania = `step()` co ~10 ms: wejścia → aktywacja (`voice-wake`) → wyniki asynchroniczne → wyjście (referencja AEC, raporty odtwarzania, `PlaybackProgress`) → mikrofon (DSP, reguła echa, VAD, koniec tury, komendy, automat) → timery → wyniki zleceń z tego kroku → zdarzenia modułów → pigułka/rezydencja → magistrala. STT, LLM, TTS i magistrala to przyszłości odpytywane raz na krok (`poll`, bez spawnowania) — krok nigdy nie czeka.
- Punkty anulowania: `CancellationToken` odpowiedzi (barge-in, „stop”, `Esc`), `CancelToken` syntezy per wypowiedź (`StopTts`), `stop_all` wyjścia, `cancel` wypowiedzi STT; po `StopTts` żadna ramka tej wypowiedzi nie trafia do miksera.

## Zależności
Tylko kontrakty: `voice-{audio,dsp,vad,stt,turn,cmd,dialog,persona,tts,wake}-contract`, `personas-contract`, `providers-contract`, `scheduler-lite-contract`, `model-residency-contract`, `core-bus-contract`, `device-profile-contract`. Rdzenie deterministyczne (`DialogMachine`, `GrammarRecognizer`, `MicArbiter`) są w kontraktach swoich modułów.

## Niezmienniki
- Najwyżej jedna agentka na głośniku (dzierżawa `scheduler-lite`); filler tylko przy wolnym głośniku albo tej samej personie.
- Mikrofon w jednym stanie naraz; strumień przechwytywania otwarty ⇔ stan słuchania ⇔ dzierżawa mikrofonu; PTT: mikrofon tylko w czasie przytrzymania.
- Własne echo TTS nie przerywa: AEC + reguła echa (przewidywane echo = szczyt referencji w oknie + nauczone sprzężenie − ERLE; mowa bliska tylko ≥ przewidywanie + margines i przy pewnej AEC).
- Historia append-only: tura asystentki zapisywana raz, po wyniku (`Completed` / `Interrupted { heard }`).
- Zdarzenia `voice.*` bez treści audio; awaria STT GPU → CPU bez utraty wypowiedzi (ponowienie z buforem).

## Zdolności / uprawnienia
Brak własnych. „Anuluj” → `voice.pipeline.cancel_task`, „stop wszystko” → `kill_switch_requested` (wykonuje watchdog/Broker).

## Izolacja
`inproc`, `lazy` (startuje z trybem głosowym).

## Budżet zasobów
Sam runtime (bez modeli): RAM ≤ 20 MB, krok ≤ 2 ms CPU na baseline (DSP/VAD liczone w swoich modułach). TTFA profilu A p50 ≤ 2000 / p95 ≤ 3000 ms; ducking < 50 ms, twardy stop ≤ 400 ms od początku mowy; „stop/czekaj” < 300 ms.

## Konfiguracja (klucze TOML)
`[voice.pipeline] tick_ms = 10`, `barge_partial_ms = 100`, `preroll_ms = 200`, `speak_ahead_ms = 8000`, `echo.enabled = true`, `echo.margin_db = 6`, `pill_every_ms = 100`, `default_persona = "alfa"`.

## Wkład do UI
Pigułka (kto mówi, poziom, transkrypt częściowy), stany mikrofonu, „przerwano tutaj” (usłyszany prefiks), zmiana agentki w locie, opóźnienia tury w Voice Lab.

## Testy akceptacyjne
- `ACC-F2-voice-pipeline-01` (CI, atrapy, wirtualny zegar): pełna tura; TTFA profilu A p50/p95 na 100 losowych scenariuszach; barge-in (ducking, twardy stop ≤ 400 ms, prefiks w historii, następna tura odpowiada na korektę); backchannel co 3 s przez 60 s → 0 przerwań; „stop”/„czekaj” < 300 ms; zmiana agentki 20/20 (głos i persona); echo własnego TTS w „pokoju” nie przerywa; PTT; awaria STT GPU → CPU; property: nigdy dwie agentki na głośniku, brak audio po `StopTts`, mikrofon w jednym stanie.
- `ACC-F2-voice-pipeline-02` (self-hosted, korpus): `evals/F2/` — WER PL, recall „stop/anuluj”, precision backchannelu, fałszywe przerwania/h, prefiks, intencje (`alfa-voice-eval`).

## Fake
`voice-pipeline-fake`: `FakePipeline` — stan mikrofonu/faza/agentka jak w potoku, skrypt na osi czasu (`ScriptStep`) → zdarzenia `voice.pipeline.*`; przechodzi test kontraktowy.

## Otwarte pytania
- Silero VAD i prawdziwy AEC (WebRTC APM) w runnerze F2 — dziś energia + whisper.cpp; pełny potok na sprzęcie mierzy Voice Lab.
- Kolejność persona/LLM przy zmianie agentki w trakcie generowania (dziś: następna tura).

## Zmiany F5 (addytywne)
- **Słowa wywoławcze** (`voice-wake` v1): `Pipeline::arm_wake_words(WakeWordListener)` / `disarm_wake_words` / `wake_listener_stats`. Po uzbrojeniu mikrofon jest otwarty, ale przed wykryciem ramki (po DSP/AEC) trafiają **tylko** do nasłuchu — 0 zdarzeń VAD/STT/transkrypcji i 0 audio na magistrali (test `e2e_wake`). Wykrycie → `WakeInput::WakeWord` → słuchanie z adresatką; koniec sesji → z powrotem nasłuch; wyciszenie/DND wstrzymują nasłuch i czyszczą bufor.
- **Weryfikacja mówcy** (`voice-speaker`): `Pipeline::set_speaker_verifier(Arc<dyn SpeakerVerifier>)`. Audio finalnej wypowiedzi idzie do weryfikacji poza krokiem; `ReplyRequest.voice: Option<VoiceProvenance { stt_confidence_permille, speaker: SpeakerCheck }>` startuje z `Pending` (albo wynikiem), wynik dochodzi przez `ReplySource::speaker_checked` (domyślnie no-op). `VoiceProvenance::command_origin()` → `CommandOrigin::UserVoice { confidence, speaker_verified }` dla `risk-classifier`/Brokera; bez wyniku/przy błędzie tura jest niezweryfikowana (reguła `VoiceUnverifiedRisky` → potwierdzenie nie-głosem). Test `e2e_speaker`.
