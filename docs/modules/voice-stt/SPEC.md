# voice-stt — SPEC (szkic v0)

## Cel
Rozpoznawanie mowy jako osobny proces: whisper.cpp (large-v3-turbo Q5_0 z bramką VAD; Vulkan/CUDA/CPU; fallback small Q5 / CPU), Parakeet v3 (ONNX) jako opcja, chmura (ElevenLabs Scribe v2 RT, Soniox, gpt-4o-transcribe, Qwen3-ASR) przez `ModelProvider`; tryb dwuprzebiegowy (szybki partial + dokładny final), biasing/hotwords, auto PL/EN, pewność per słowo (PLAN §6.2, §6.3, §1.2).

## Fala i priorytet
F0: spike (a)(h) pomiary; F2: moduł (lokalny + chmura po kluczu). P0.

## Kontrakt (szkic Rust)
```rust
// voice-stt-contract — SZKIC
pub struct SttCfg { pub engine: SttEngine /* WhisperCpp { model, backend } | Parakeet | Cloud(AccountId, ModelId) */, pub langs: Vec<Lang> /* pl, en, auto */,
                    pub hotwords: Vec<String>, pub two_pass: bool, pub privacy: PrivacyTag }
pub struct Transcript { pub text: String, pub words: Vec<Word { text, start, end, confidence }>, pub lang: Lang, pub is_final: bool, pub confidence: f32, pub latency: Duration }
pub trait Stt: Send + Sync {
    fn configure(&self, cfg: SttCfg) -> Result<()>;
    fn start_utterance(&self, id: UtteranceId) -> Result<()>;
    fn push(&self, id: UtteranceId, frame: &Frame) -> Result<()>;                    // 16 kHz mono po DSP, tylko gdy VAD = mowa
    fn end_utterance(&self, id: UtteranceId) -> BoxStream<Transcript>;               // partial…final
    fn cancel(&self, id: UtteranceId);
    fn health(&self) -> Health;
}
```
Zdarzenia: `stt.partial`, `stt.final` (WER-metryki w strumieniu Voice), `stt.backend.fallback` (Vulkan → CPU), `stt.model.loaded/unloaded`, `stt.cloud.sent` (audio poszło do chmury — ekran „co poszło do chmury").

## Zależności
`core-bus/config/log-contract`, `voice-dsp-contract`, `voice-vad-contract`, `model-residency-contract` (VRAM 1–2,5 GB), `providers-api-contract` (chmura), `device-profile-contract` (backend), `platform-windows-contract` (sidecar). Zewnętrzne: whisper.cpp ≥ 1.8.1 przypięte (`docs/vendor/whisper-cpp.md`), sherpa-onnx.

## Niezmienniki
- Lokalny STT to osobny proces (crash nie zabija aplikacji; automatyczny fallback backendu, ograniczona liczba prób).
- Audio do STT tylko przy VAD = mowa (bramka VAD — turbo halucynuje na szumie).
- Pewność STT (per słowo i utterance) zawsze dostępna — jest wejściem `risk-classifier` (PLAN §6.10).
- Audio idzie do chmury tylko gdy tag prywatności sesji na to pozwala i konto skonfigurowane; każdorazowo zdarzenie `stt.cloud.sent`.
- Modele tylko z hashem; brak kwantów IQ; wersja whisper.cpp i sterownik przypięte (ryzyko Vulkan/RDNA).
- Partial może być nadpisany przez final; final jest niezmienny (do dziennika).

## Zdolności / uprawnienia
`net.egress(host)` dla silników chmurowych (per konto); brak dla lokalnych.

## Izolacja
`process` (JSON-RPC po pipe; Job Object), `on-demand` z `idle_unload`; rezydencja przez `model-residency`.

## Budżet zasobów
Baseline: turbo Q5_0 (547 MiB) w VRAM 1–2,5 GB; finalizacja 150–300 ms lokalnie (§6.4); RAM sidecara ≤ 1,5 GB; CPU-fallback: small Q5 (czasy best effort).

## Konfiguracja (klucze TOML)
`[voice.stt] engine = "whisper_cpp"`, `model = "large-v3-turbo-q5_0"`, `langs = ["pl", "en"]`, `two_pass = true`, `hotwords = []`; per maszyna: `[machine.voice.stt] backend = "auto"`, `fallback_model = "small-q5"`, `idle_unload = "10m"`; chmura: `[voice.stt.cloud] account, model`.

## Wkład do UI
Napisy na żywo (szary partial → pełny final), dyktowanie do czatu z podglądem, Ustawienia → Głos → STT (silnik, model, hotwords, backend), stan GPU OOM.

## Testy akceptacyjne
- `ACC-F0-voice-stt-01`: spike (e)/(h): WER PL ≤ 12% na korpusie własnym; 0 crashy w 1 h na Vulkan (desktop) i CUDA (laptop); VRAM w budżecie.
- `ACC-F2-voice-stt-02`: WER PL ≤ 12% na zamrożonym zestawie testowym (≥ 300 wypowiedzi, cisza/szum/głośniki/słuchawki).
- `ACC-F2-voice-stt-03`: wymuszony błąd GPU → fallback CPU bez utraty bieżącej wypowiedzi.
- `ACC-F2-voice-stt-04`: sesja „prywatne" → 0 wywołań chmurowego STT (100 prób).

## Fake
`voice-stt-fake`: transkrypty z adnotacji korpusu (partial/final z czasami, pewności), sterowane opóźnienie z wirtualnym zegarem, symulacja crashu sidecara.

## Otwarte pytania
- Parakeet v3 jako szybki partial (CPU) + whisper jako final — czy warto na baseline; Voice Lab F2.
- Protokół sidecara wspólny z `providers-local` — do ustalenia w SPEC v1.
