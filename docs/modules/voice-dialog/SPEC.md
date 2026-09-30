# voice-dialog — SPEC (szkic v0)

## Cel
Automat rozmowy głosowej: `Idle → Listening → UserSpeaking → Thinking → Speaking → Interrupted → …`; zatrzymanie dwustopniowe (ducking → twardy stop), keyword-spotter „stop/czekaj" z `voice-cmd`, backchannel, klasy intencji przerwania, „usłyszany prefiks", wznawianie, fillery, mowa proaktywna z etykietą, kolejka mówienia; przerwanie także tekstem i w trakcie narzędzi (PLAN §6.5, §6.9).

## Fala i priorytet
F0: spike (a) pętla z barge-in; F2: moduł. P0.

## Kontrakt (szkic Rust)
```rust
// voice-dialog-contract — SZKIC
pub enum DialogState { Idle, Listening, UserSpeaking, Thinking, Speaking { persona: PersonaId, utterance: UtteranceId }, Interrupted { heard_prefix: usize, approximate: bool } }
pub enum InterruptIntent { Correction, Addition, Clarify, TopicChange, StopCancel, Continue, Backchannel }
pub struct BargeInPolicy { pub duck_db: f32, pub confirm_ms: u16 /* 150–250 */, pub headphones_aggressive: bool, pub no_single_word_nie_outside_speaking: bool }
pub trait Dialog: Send + Sync {
    fn state(&self) -> DialogState;
    fn on_vad(&self, ev: VadEvent, aec_conf: f32);
    fn on_turn_end(&self, t: Transcript);
    fn on_text_input(&self, text: String);                   // przerwanie pisaniem
    fn stop_speech(&self);                                   // Esc / „stop" — nie zabija pracy w tle
    fn resume(&self);                                        // od punktu cięcia
    fn proactive(&self, persona: PersonaId, text: String, label: ProactiveLabel) -> Result<()>;
}
```
Zdarzenia: `dialog.state_changed`, `dialog.ducked`, `dialog.interrupted { heard_prefix, approximate }`, `dialog.intent_classified`, `dialog.backchannel`, `dialog.proactive`, `dialog.filler`, `dialog.metrics` (p50/p95 etapów, fałszywe przerwania — strumień Voice).

## Zależności
`core-bus/config/log-contract`, `voice-audio/dsp/vad/turn/stt/tts/cmd/persona-contract`, `scheduler-lite-contract` (`speaker`, `mic` jako zasoby wyłączne), `personas-contract` (Mówczyni/obsada), `router-contract` (VoiceFast/Conversation), `sessions-contract` (tury, prefiks, gałęzie), `agent-runtime-contract` (F3: steering, anulowanie LLM).

## Niezmienniki
- Ducking (−15 dB, < 50 ms) przy VAD po AEC w `Speaking`; twardy stop po ≥ 150–250 ms mowy sklasyfikowanej ≠ backchannel: stop TTS, anulowanie LLM, czyszczenie kolejki — ≤ ~400 ms od początku wypowiedzi.
- „nie" przerywa tylko jako samodzielne słowo z pauzą przed i po, wyłącznie w `Speaking`.
- Prefiks usłyszany liczony wg hierarchii (znaczniki → alignment → próbki + `GetStreamLatency`), zawsze z flagą `approximate`; `assistant_full` i `heard_prefix` zapisywane osobno; historia append-only (nowe gałęzie, nigdy edycja).
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
`voice-dialog-fake`: automat sterowany skryptem zdarzeń (VAD/turn/STT/TTS z fake'ów) na wirtualnym zegarze; udostępnia stany dla UI i `agent-runtime`.

## Otwarte pytania
- Model klasyfikacji intencji przerwania (mały lokalny vs LLM) — pomiar F2; do ustalenia w SPEC v1.
- Współpraca z natywnym truncate (OpenAI Realtime) — F5 (`voice-s2s`).
