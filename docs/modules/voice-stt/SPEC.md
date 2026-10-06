# voice-stt — SPEC (v0, zaimplementowany w F2)

## Cel
Rozpoznawanie mowy jako osobny proces: whisper.cpp (large-v3-turbo Q5_0 z bramką VAD; Vulkan/CUDA/CPU; fallback small Q5 / CPU), Parakeet v3 (ONNX) jako opcja, chmura (ElevenLabs Scribe v2 RT, Soniox, gpt-4o-transcribe, Qwen3-ASR) przez `ModelProvider`; tryb dwuprzebiegowy (szybki partial + dokładny final), biasing/hotwords, auto PL/EN, pewność per słowo (PLAN §6.2, §6.3, §1.2).

## Fala i priorytet
F0: spike (a)(h) pomiary; F2: moduł (lokalny + chmura po kluczu). P0.

## Kontrakt (Rust, `voice-stt-contract`)
```rust
pub struct SttCfg { pub engine: SttEngine /* WhisperCpp { model, backend } | Parakeet | Cloud { provider, account, model } */,
                    pub language: LangMode /* auto | pl | en */, pub hotwords: Vec<String>, pub two_pass: TwoPass, pub privacy: PrivacyTag, pub min_speech_ms: u32 }
pub struct Transcript { pub utterance: UtteranceId, pub text: String, pub words: Vec<Word { text, start_ms, end_ms, confidence }>, pub lang: String,
                        pub is_final: bool, pub confidence: f32, pub latency_ms: u32, pub backend: Option<Backend> }
#[async_trait] pub trait Stt: Send + Sync {
    async fn configure(&self, cfg: SttCfg) -> Result<(), SttError>;          // prywatność sprawdzana przed ruchem sieciowym
    async fn start_utterance(&self, id: UtteranceId) -> Result<(), SttError>;
    async fn push(&self, id: UtteranceId, frame: &Frame) -> Result<Option<Transcript>, SttError>;   // partial co partial_every_ms
    async fn partial_now(&self, id: UtteranceId) -> Result<Option<Transcript>, SttError>;          // partial na żądanie (KWS barge-in potoku co 100 ms); domyślnie None
    async fn end_utterance(&self, id: UtteranceId) -> Result<Transcript, SttError>;                 // final (pusty, gdy bramka VAD)
    async fn cancel(&self, id: UtteranceId);  fn health(&self) -> Health;  fn take_events(&self) -> Vec<SttEvent>;
}
```
Zdarzenia: `voice.stt.partial`, `voice.stt.final`, `voice.stt.backend.fallback` (Vulkan/CUDA → CPU), `voice.stt.model.loaded/unloaded`, `voice.stt.cloud.sent` (ekran „co poszło do chmury”), `voice.stt.gate_rejected`.

## Zależności
`core-bus/config/log-contract`, `voice-dsp-contract`, `voice-vad-contract`, `model-residency-contract` (VRAM 1–2,5 GB), `providers-contract` (tag prywatności; chmura), `device-profile-contract` (backend), `platform-windows-contract` (sidecar). Zewnętrzne: whisper.cpp ≥ 1.8.1 przypięte (`whisper-server`, `docs/vendor/whisper-cpp.md`), `reqwest` 0.12 (HTTP 127.0.0.1); sherpa-onnx (Parakeet) — kandydat.

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
`process` (HTTP na 127.0.0.1 — `whisper-server`; Job Object przez `ProcessPort` przy integracji), `on-demand` z `idle_unload`; rezydencja przez `model-residency`.

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

## Decyzje v0 (F2)
- Sidecar = `whisper-server` (HTTP na 127.0.0.1, losowy port; `/health`, `/inference` z `verbose_json`) zamiast JSON-RPC po pipe — gotowy protokół whisper.cpp; proces bez okna, zabijany przy porzuceniu.
- Partiale z polityki dwóch przebiegów (`push` co 1 s audio, wiązka 1) zamiast strumienia z `end_utterance`; final z wiązką 5.
- Fallback: awaria procesu GPU (wyjście, zerwane połączenie, `ErrorDeviceLost` w stderr) → backend oznaczony, restart na CPU (`-ng`), wypowiedź ponowiona z bufora.
- Chmura: typy i konfiguracja; adapter w kolejnej fali (`NotAvailable`), sesja prywatna → `PrivacyBlocked`.

## Fala 6 (próba generalna, laptop z RTX 4050)
- Nieudany **start** kompilacji GPU (proces kończy się od razu — np. wersja CUDA bez sterownika NVIDIA albo bez
  `cudart`) → backend oznaczony jako niesprawny, `voice.stt.backend.fallback` i start na CPU w tej samej wypowiedzi
  (`engine_start.rs`); wcześniej rozpoznawanie zwracało błąd aż do restartu. Test `gpu_start_failure_falls_back_to_cpu`.
- Kompozycja w aplikacji (`app_modules::stt::whisper`, używa jej `app-voice` i test na żywo `app-models`): `whisper-server`
  CUDA (`sidecars/whisper-cuda/`, pozycja menedżera `sidecar-whisper-cuda`) ma pierwszeństwo, gdy jest zainstalowany
  (PLAN §6.3 D-CUDA), CPU (`sidecars/whisper/`) wymagany jako zapas; wątki CPU: połowa wątków logicznych, 4–8
  (wcześniej stałe 4). Build Vulkan whisper.cpp nie istnieje w wydaniach — desktop AMD rozpoznaje na CPU.
- **STT obok lokalnego LLM na karcie 6 GB:** dzierżawa `whisper-server` na GPU ma umiejscowienie `GpuIfFree`
  (`model-residency`): karta tylko z wolnego miejsca, inaczej CPU — STT nie wypiera lokalnego LLM (który przeładowałby
  się na CPU w tej samej rozmowie). Przy domyślnym kontekście LLM dobranym przez `providers-local` (laptop: 4096) oba
  mieszczą się na karcie. Test `stt_never_evicts_the_conversation_model_on_a_6_gb_laptop`.


## Zmiany — wyjście `whisper-server` w dzienniku (2026-10-06)
- Wiersze stderr `whisper-server` trafiają do dziennika Alfy (`sidecar::classify_line`): błędy i utrata urządzenia
  (`DEVICE_LOST_MARKERS`, `out of memory`) jako `warn`, model i backend (`using CUDA0 backend`, `model size`) jako `info`
  (cel `voice_stt_impl::whisper_server`, domyślnie zapisywane); reszta `debug` pod celem `whisper_server` (tylko po jawnym
  włączeniu). Wiersze z transkrypcją (`[… --> …]`) nigdy wyżej niż `debug` — treść rozmowy nie trafia do dziennika.
  Wcześniej stderr służył tylko do wykrywania utraty urządzenia (ostatnie 64 wiersze w pamięci).
- **Partiale adaptacyjne:** kolejny partial najwcześniej po `max(partial_every_ms, 2 × czas poprzedniego partiala)`
  mowy. Na runnerze CI (CPU 4 vCPU, `ggml-small-q5_1`, partial co 1 s) rozpoznanie 2,1 s mowy trwało 71 s — każdy
  przebieg whispera na CPU to kilka sekund, a partiale szły jeden za drugim przed finalem. Na karcie (partial ≪ 1 s)
  odstęp bez zmian. Test: `tests/stt.rs::slow_partials_back_off_so_they_never_hog_the_engine`.
