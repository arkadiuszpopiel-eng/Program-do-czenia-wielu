# voice-dialog — SPEC (v1: kontrakt, fake i impl w repo; F2)

## Cel
Automat rozmowy głosowej: `Idle → Listening → UserSpeaking → Thinking → Speaking → Interrupted → …`; zatrzymanie dwustopniowe (ducking → twardy stop), keyword-spotter „stop/czekaj" z `voice-cmd`, backchannel, klasy intencji przerwania, „usłyszany prefiks", wznawianie, fillery, mowa proaktywna z etykietą, kolejka mówienia; przerwanie także tekstem i w trakcie narzędzi (PLAN §6.5, §6.9).

## Fala i priorytet
F0: spike (a) pętla z barge-in; F2: moduł. P0.

## Kontrakt (v1, `crates/voice-dialog-contract`)
Automat **czysto funkcyjny**, bez I/O: `DialogAutomaton::step(&DialogState, &DialogEvent, now_ms) -> Transition { state, commands }`.
```rust
pub enum DialogPhase { Idle, Listening, UserSpeaking, Thinking, Speaking, Interrupted }   // agent_activity() → voice-cmd
pub enum DialogEvent { Activate, Deactivate, IdleTimeout, VadSpeechStart, VadSpeechEnd, UserPartial { text }, TurnEnded,
    Command { VoiceCommand }, UserTyped { text }, ResponseReady { persona }, SpeakerGranted/Denied { persona, utterance }, SpeakerReleased,
    TtsChunkQueued { utterance, text, audio_ms }, TtsWordMarks { utterance, chunk, marks, source: Tts|Alignment },
    PlaybackProgress { utterance, played_samples, sample_rate, device_latency_ms }, ResponseFinished { utterance },
    ProactiveRequest { persona, text, label }, SetDoNotDisturb { enabled }, StopSpeech /* Esc */, Tick }
pub enum Command { StartListening, StopListening, DuckOutput { db }, RestoreOutput, StopTts { utterance }, CancelGeneration,
    ClearSpeechQueue, CancelTask, AcquireSpeaker/ReleaseSpeaker { persona, utterance }, StartTts, SpeakProactive { label, .. },
    ResumeFrom { from, offset, text, utterance }, SubmitTurn { turn, text, heard_prefix, interrupted_intent, source: Voice|Text },
    PlayFiller, StopFiller, ForwardCommand { VoiceCommand }, KillSwitch, Notify { DialogNotice } }
pub struct HeardPrefix { utterance, chars, words, text, approximate, source: WordMarks|Alignment|SampleCount|NothingPlayed }
pub enum InterruptIntent { Correction, Addition, Clarify, TopicChange, StopCancel, Continue, Backchannel }
pub trait InterruptClassifier { fn classify(&self, &InterruptContext { heard_prefix, unsaid, utterance }) -> IntentResult; }
pub trait SpeakerLock { fn try_acquire(&SpeakerOwner) -> Result<(), SpeakerBusy>; fn release(&SpeakerOwner) -> bool; fn holder() -> Option<SpeakerOwner>; }
pub trait WordAligner { fn align(&self, text, audio: &[f32], sample_rate) -> Result<Vec<WordMark>, AlignError>; }
```
`DialogConfig`: `duck_db −15`, `confirm_ms 200` (słuchawki 150), `max_confirm_ms 350` (brak transkryptu), `backchannel_max_ms 900`, `min_speech_ms 80` (szum), frazy backchannelu (m.in. „mhm”, „tak”, „aha”, „okej”, „nie no, dobrze”), `approx_trim word|sentence`, fillery po 1200 ms, `proactive labeled|off`.
Impl: `DialogMachine<C>` (+ `HeuristicClassifier` PL, `DialogDriver` wykonujący polecenia głośnika na `SpeakerLock`). Id wypowiedzi nadaje automat; zdarzenia nieaktualnych wypowiedzi są ignorowane (po `StopTts` brak dalszego audio tej wypowiedzi; wznowienie = nowa wypowiedź).
Zdarzenia magistrali: `voice.dialog.state_changed`, `.ducked`, `.interrupted`, `.intent_classified`, `.backchannel`, `.proactive`, `.filler`, `.metrics` (z `Command::Notify`).

## Zależności
v1: `core-bus-contract`, `voice-cmd-contract` (komendy), `voice-persona-contract` (`PersonaId`). Automat nie zależy od `voice-turn` ani VAD — dostaje zdarzenia `VadSpeech*`/`TurnEnded` od runtime potoku. Później: `scheduler-lite-contract` (realny `SpeakerLock`), `sessions-contract` (tury, prefiks, gałęzie), `router-contract`, `agent-runtime-contract` (F3: steering, anulowanie LLM).

## Niezmienniki
- Ducking (−15 dB, < 50 ms) przy VAD po AEC w `Speaking`; twardy stop po ≥ 150–250 ms mowy sklasyfikowanej ≠ backchannel: stop TTS, anulowanie LLM, czyszczenie kolejki — ≤ ~400 ms od początku wypowiedzi.
- „nie" przerywa tylko jako samodzielne słowo z pauzą przed i po, wyłącznie w `Speaking`.
- Prefiks usłyszany liczony wg hierarchii (znaczniki TTS → alignment (`WordAligner` → `TtsWordMarks{source: Alignment}`) → próbki − `GetStreamLatency`, przycięcie do słowa/zdania), zawsze z flagą `approximate`; `assistant_full` i `heard_prefix` zapisywane osobno; historia append-only (nowe gałęzie, nigdy edycja).
- Twardy stop: treść ≠ backchannel po `confirm_ms`; brak transkryptu po `max_confirm_ms`; backchannel/prefiks backchannelu dłuższy niż `backchannel_max_ms`. „Czekaj/pauza” zatrzymuje mowę bez anulowania generowania (wznowienie); „stop/Esc” anuluje generowanie mówionej odpowiedzi; „anuluj” dodatkowo `CancelTask`.
- Backchannel („mhm", „tak") nie przerywa; fillery poza prefiksem i przerywalne; mowa proaktywna nigdy podczas mowy użytkownika ani w DND.
- Jedna agentka mówi naraz; przekazanie głosu jawne („Przekazuję Delcie…").
- Stop mowy ≠ Stop wszystkiego (kill-switch obsługuje watchdog/broker).
- Brak LLM w ścieżce stop/pauza (to `voice-cmd`).

## Zdolności / uprawnienia
Brak własnych; destrukcyjne akcje wywołane głosem wymagają potwierdzenia nie-głosem w Broker-UI na każdym poziomie (egzekwuje `safety-broker`; `dialog` tylko odczytuje treść potwierdzenia).

## Izolacja
`inproc`, `lazy` (z potokiem głosu).

## Budżet zasobów
RAM ≤ 5 MB; reakcja automatu ≤ 5 ms; łączne opóźnienie profilu A p50 ≤ 2000 ms / p95 ≤ 3000 ms, B p50 ≤ 1300 / p95 ≤ 2000, C p50 ≤ 900 / p95 ≤ 1500 (§6.4, wstępne).

## Konfiguracja (klucze TOML)
`[voice.dialog] barge_in.confirm_ms = 200`, `barge_in.duck_db = -15`, `backchannel_agent = true`, `fillers = true`, `proactive = "labeled" | "off"`, `verbosity = "short"`, `patience` (→ voice-turn), `intent_model = "local_small" | "llm"`.

## Wkład do UI
Pełny tryb głosowy (orb, napisy, „przerwano tutaj", stany mikrofonu), pigułka, `Esc` (stop mowy), makieta 3; Ustawienia → Głos → Tury i barge-in.

## Testy akceptacyjne
- `ACC-F0-voice-dialog-01`: spike (a): profil A p50 ≤ 2000 ms, p95 ≤ 3000 ms (desktop z emulacją + korekta GPU, laptop).
- `ACC-F2-voice-dialog-02`: recall „stop/anuluj" ≥ 99% na ≥ 200 próbach, reakcja < 300 ms; precision backchannelu ≥ 95%.
- `ACC-F2-voice-dialog-03`: fałszywe przerwania ≤ 1/godz. (1 h TTS przez głośniki laptopa + tło TV).
- `ACC-F2-voice-dialog-04`: prefiks ±1 słowo ≥ 90%; klasyfikacja intencji ≥ 90% per klasa (≥ 50 przykładów na klasę).

## Fake
`voice-dialog-fake`: `FakeDialog` (uproszczony automat: natychmiastowy stop przy mowie, intencja zawsze korekta), `FakeSpeakerLock`, `ScriptedClassifier`, `UniformAligner`; skrypt zdarzeń na wirtualnym zegarze przez `drive`; przechodzi test kontraktowy.

## Stan testów (v1, scenariusze syntetyczne)
Twardy stop od początku mowy: p50 220 ms, p95/max 350 ms (120 scenariuszy; ducking w tym samym kroku co VAD). Backchannel co 3 s przez 60 s: 0 fałszywych przerwań (20/20 backchanneli rozpoznanych). Prefiks ±1 słowo: znaczniki 100 %, liczenie próbek 93 %. Klasyfikator heurystyczny: 100 % per klasa na tabeli deweloperskiej (7–12 przykładów/klasę; oficjalny zestaw ≥ 50/klasę — recenzent). Property-based (768 przypadków): brak „mówi+słucha” bez duckingu, brak audio po `StopTts`, głośnik tylko w `Speaking`, mowa proaktywna tylko z `Idle` bez DND.

## Otwarte pytania
- Model klasyfikacji intencji przerwania (mały lokalny vs LLM) — pomiar F2; do ustalenia w SPEC v1.
- Współpraca z natywnym truncate (OpenAI Realtime) — F5 (`voice-s2s`).
