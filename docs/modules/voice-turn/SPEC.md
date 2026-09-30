# voice-turn — SPEC (v1: kontrakt, fake i impl w repo; F2)

## Cel
Wykrywanie końca tury użytkownika: Smart Turn v3.2 (PL wśród 23 języków, ~8 MB, ~10 ms CPU) + polityka cierpliwości (dłużej czeka po hezytacjach „yyy", regulowana), by odpowiedź startowała szybko, ale nie w środku wypowiedzi (PLAN §6.2, §6.4, §6.5).

## Fala i priorytet
F2. P0. Budżet etapu: 250–500 ms (§6.4).

## Kontrakt (v1, `crates/voice-turn-contract`)
```rust
pub enum Patience { Low, Normal, High, Custom(PatienceCfg) }   // Normal: min 200, base 300, hesitation +400, low_prob +500, max 1500 ms
pub struct TurnCfg { patience, use_partial_text, eot_threshold /*0.5*/, confident_threshold /*0.85*/ }   // validate()
pub enum TurnEvent { SpeechStart { at_ms }, SpeechEnd { at_ms }, Partial { at_ms, text }, Reset }
pub enum TurnDecision { Idle, Wait { until_ms, reason: UserSpeaking|Silence|Hesitation|ModelUnsure }, EndOfTurn { at_ms, confidence, reason: ClearEnd|Patience|MaxSilence } }
pub trait TurnModel { fn name(&self) -> &str; fn end_probability(&self, input: &TurnModelInput /* audio_tail?, partial?, silence_ms */) -> Result<f32>; }
pub trait TurnDetector: Send { fn configure(&mut self, TurnCfg) -> Result<()>; fn config(&self) -> &TurnCfg;
    fn observe(&mut self, &TurnEvent); fn decide(&mut self, now_ms: u64, audio: Option<AudioTail>) -> TurnDecision; }
```
Polityka (`PatienceTurnDetector`): wymagana cisza = `base`; model ≥ 0,85 → `min_silence`; model < 0,5 → `+low_prob`; bez modelu a z interpunkcją końcową → `min_silence`; hezytacja („yyy”, „eee”, „znaczy”, „hmm”, „mhm”, niedokończone „i/że/bo”, przyimek, przecinek, wielokropek) → `+hesitation`; zawsze w `[min_silence, max]`. Wynik modelu liczony raz na (koniec mowy, wersja transkryptu); błąd modelu → sama polityka. `HeuristicTurnModel` (tekstowy) zastępuje Smart Turn do czasu impl ONNX. `Uncertain` ze szkicu zastąpione przez `Wait { reason }`.
Zdarzenia: `voice.turn.end`, `voice.turn.hesitation`, `voice.turn.model_loaded/unloaded`.

## Zależności
`core-bus/config/log-contract`, `voice-vad-contract` (SpeechEnd, cisza), `voice-stt-contract` (partial), `model-residency-contract` (mały model CPU). Zewnętrzne: Smart Turn (ONNX, hash), sherpa-onnx/ONNX Runtime CPU.

## Niezmienniki
- Tylko CPU; decyzja ≤ 30 ms na baseline (model ~10 ms).
- `EndOfTurn` nie wcześniej niż `min_silence` z VAD i nie później niż `max_ms` cierpliwości (twardy limit — brak zawieszenia w „czekaniu").
- Hezytacje („yyy", „hmm", niedokończone zdanie wg partial STT) wydłużają oczekiwanie, ale nigdy ponad `max_ms`.
- Backchannel użytkownika w stanie `Speaking` agentki nie jest turą (klasyfikuje `voice-dialog`).
- Cierpliwość regulowana w UI i głosem („poczekaj dłużej, zanim odpowiesz").

## Zdolności / uprawnienia
Brak.

## Izolacja
`inproc`, `lazy`.

## Budżet zasobów
CPU: ≤ 10–15 ms na decyzję; RAM ≤ 40 MB; etap końca tury łącznie 250–500 ms (cel §6.4).

## Konfiguracja (klucze TOML)
`[voice.turn] patience = "normal"`, `base_ms = 300`, `hesitation_bonus_ms = 400`, `max_ms = 1500`, `use_partial_text = true`.

## Wkład do UI
Suwak cierpliwości (Ustawienia → Głos → Tury i barge-in); w trybie głosowym stan „przetwarza" po końcu tury.

## Testy akceptacyjne
- `ACC-F2-voice-turn-01`: na korpusie (≥ 300 wypowiedzi PL + PL/EN, zestaw zamrożony): trafność końca tury ≥ 95%, przedwczesne cięcia ≤ 3% (progi do potwierdzenia w ACCEPTANCE).
- `ACC-F2-voice-turn-02`: udział etapu w p50/p95 profilu A ≤ 500 ms (pomiar Voice Lab z wirtualnym zegarem i na sprzęcie).
- `ACC-F2-voice-turn-03`: hezytacje z korpusu — ≥ 90% nie przerwane przed dokończeniem.

## Fake
`voice-turn-fake`: `ScriptedTurnDetector` (koniec tury po adnotowanej ciszy), `ScriptedTurnModel` (prawdopodobieństwa/błędy z kolejki); przechodzi test kontraktowy.

## Stan testów (v1)
Kontrakt (impl z modelem heurystycznym i skryptowym, fake), scenariusze z wirtualnym zegarem (pytanie → 200 ms, hezytacja → 1200 ms, twardy limit, poziomy cierpliwości, błąd modelu), property-based (nigdy koniec w trakcie mowy, zawsze w granicach, raz na turę).

## Otwarte pytania
- Wersja Smart Turn i format modelu (ONNX/CoreML) do przypięcia — `docs/vendor/`; jakość PL — Voice Lab.
