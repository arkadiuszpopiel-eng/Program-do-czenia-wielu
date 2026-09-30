# voice-turn — SPEC (szkic v0)

## Cel
Wykrywanie końca tury użytkownika: Smart Turn v3.2 (PL wśród 23 języków, ~8 MB, ~10 ms CPU) + polityka cierpliwości (dłużej czeka po hezytacjach „yyy", regulowana), by odpowiedź startowała szybko, ale nie w środku wypowiedzi (PLAN §6.2, §6.4, §6.5).

## Fala i priorytet
F2. P0. Budżet etapu: 250–500 ms (§6.4).

## Kontrakt (szkic Rust)
```rust
// voice-turn-contract — SZKIC
pub struct TurnCfg { pub patience: Patience /* Low | Normal | High | Custom { base_ms, hesitation_bonus_ms, max_ms } */, pub model: TurnModel /* SmartTurnV3 */ }
pub struct TurnInput<'a> { pub audio_tail: &'a [f32] /* ostatnie ~8 s po VAD */, pub partial_text: Option<&'a str> /* z STT dwuprzebiegowego */, pub silence_ms: u16 }
pub enum TurnDecision { Continue { wait_ms: u16 }, EndOfTurn { confidence: f32, ts: Instant }, Uncertain { ask_after_ms: u16 } }
pub trait TurnDetector: Send + Sync {
    fn configure(&self, cfg: TurnCfg) -> Result<()>;
    fn decide(&self, input: TurnInput) -> TurnDecision;      // wywoływane po VAD SpeechEnd i cyklicznie w ciszy
    fn reset(&self);
}
```
Zdarzenia: `turn.end` (pewność, czas od ostatniej ramki mowy), `turn.hesitation` (wydłużona cierpliwość), `turn.model.loaded/unloaded`.

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
`voice-turn-fake`: decyzje z adnotacji (czas końca tury per wypowiedź) — deterministyczne testy `voice-dialog`.

## Otwarte pytania
- Wersja Smart Turn i format modelu (ONNX/CoreML) do przypięcia — `docs/vendor/`; jakość PL — Voice Lab.
