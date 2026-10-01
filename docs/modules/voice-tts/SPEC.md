# voice-tts — SPEC (v0, zaimplementowany w F2)

## Cel
Synteza mowy jako osobny proces: lokalnie na baseline Pocket TTS + model PL społeczności (CPU, RTF ≈ 0,21, ~200 ms do 1. fragmentu, klon z krótkiej referencji), Piper pl_PL jako zapas; poza baseline (profil D-CUDA) Chatterbox/XTTS-v2/F5; chmura (ElevenLabs, Cartesia, Google, Azure, Gemini, OpenAI, MiniMax) przez `ModelProvider`. Chunker, streaming zdanie-po-zdaniu, dostawca znaczników słów, cache fraz stałych, łańcuch fallback per agentka (PLAN §6.2, §6.3, §6.6–6.7).

## Fala i priorytet
F0: spike (e) tabela kandydatów; F2: moduł z głosami v0 (≥ 2 bazowe mówczynie + wysokość/tempo, bez kluczy); casting właściwy po kluczu. P0.

## Kontrakt (Rust, `voice-tts-contract`)
```rust
pub struct VoiceRef { pub persona: PersonaId, pub engine: TtsEngine /* Pocket | Piper | Chatterbox | Xtts | Cloud { provider, account, voice } */,
                      pub preset: VoicePreset { base_speaker, pitch, rate }, pub reference: Option<PathBuf> }
pub struct TtsRequest { pub utterance: u64, pub persona: PersonaId, pub text: String, pub style: SpeechStyle, pub cacheable: bool, pub privacy: PrivacyTag }
pub struct TtsChunk { pub utterance: u64, pub seq: u32, pub audio: Frame /* 24 kHz */, pub marks: Vec<WordMark { word_idx, word, start_ms, end_ms }>,
                      pub marks_kind: MarksKind /* Native | ForcedAlign | Estimated */, pub is_last: bool, pub engine: String }
#[async_trait] pub trait Tts: Send + Sync {
    fn voices(&self) -> Vec<VoiceInfo>;
    async fn synth(&self, req: TtsRequest, cancel: CancelToken) -> Result<TtsStream /* mpsc::Receiver<Result<TtsChunk, TtsError>> */, TtsError>;
    fn stop(&self, utterance: u64);  async fn warm(&self, persona: &PersonaId) -> Result<(), TtsError>;
    fn health(&self) -> TtsHealth;  fn take_events(&self) -> Vec<TtsEvent>;
}
// Wspólne: split_sentences (chunker PL), estimate_marks, v0_chains, validate_chains (odrębność brzmień).
```
Zdarzenia: `voice.tts.started` (TTFB), `voice.tts.chunk` (Diagnostyka), `voice.tts.finished`, `voice.tts.stopped`, `voice.tts.fallback` (silnik zapasowy, per agentka), `voice.tts.cloud.sent`.

## Zależności
`core-bus/config/log-contract`, `voice-audio-contract` (odtwarzanie, referencja AEC), `voice-persona-contract` (biblia głosu, normalizator, styl→silnik), `model-residency-contract`, `providers-api-contract` (chmura), `device-profile-contract`. Zewnętrzne: Pocket TTS (CC-BY-4.0), Piper, whisper.cpp/aligner dla forced alignment (`docs/vendor/`).

## Niezmienniki
- Jedna wypowiedź naraz na wyjściu (koordynuje `scheduler-lite`/`voice-dialog`); `stop` ucisza ≤ 20 ms i anuluje generowanie.
- Każdy chunk niesie znaczniki słów (natywne → forced alignment → estymata z licznika próbek, z flagą) — hierarchia „usłyszanego prefiksu" (PLAN §6.5).
- Głos zapasowy agentki nie może brzmieć jak inna agentka (łańcuch fallback per persona, odrębność sprawdzana ECAPA).
- Brak klonowania prawdziwych osób ani lektorów z korpusów; referencje tylko z castingu z zapisaną zgodą/pochodzeniem.
- Bez Pythona na baseline; Chatterbox/XTTS/F5 tylko przy CUDA (profil D) jako opcjonalne moduły.
- Audio do chmury tylko zgodnie z tagiem prywatności; zdarzenie `tts.cloud.sent`.

## Zdolności / uprawnienia
`net.egress(host)` dla silników chmurowych; brak dla lokalnych.

## Izolacja
`process` (Pocket: JSON-lines po stdio; Piper: proces na zdanie), `on-demand` z `idle_unload`; 24–48 kHz wyjście.

## Budżet zasobów
Baseline: Pocket-PL na CPU RTF ≈ 0,21 przy 6 rdzeniach (pomiar spike h), TTFB 200–500 ms lokalnie, 75–300 ms chmura (§6.4); RAM 0,5–1,5 GB; cache fraz ≤ 50 MB na dysku.

## Konfiguracja (klucze TOML)
`[voice.tts] default_engine = "pocket"`, `sample_rate = 24000`, `cache_phrases = true`; `[voice.tts.persona.<id>] chain = ["pocket:alfa", "piper:pl_PL-x", "cloud:elevenlabs:<voice>"]`, `preset = { base = "...", pitch = 0.0, rate = 1.0 }`; per maszyna: `[machine.voice.tts] engine_override`, `idle_unload = "5m"`.

## Wkład do UI
Napisy z podświetleniem wypowiedzianych słów, „przeczytaj na głos" (głosem agentki), Ustawienia → Głos → TTS, Voice Lab (casting, ślepe A/B, makieta 16).

## Testy akceptacyjne
- `ACC-F0-voice-tts-01`: spike (e): ślepa ocena właściciela 1–5 na 20 zdaniach PL, średnia ≥ 4,0 (no-go = zostają głosy v0; nie blokuje F1).
- `ACC-F2-voice-tts-02`: TTFB p50/p95 w budżecie §6.4 na baseline emulowanym (Voice Lab).
- `ACC-F2-voice-tts-03`: odrębność głosów: cos-sim ECAPA między parami ≤ 0,6 + identyfikacja ABX właściciela ≥ 90%.
- `ACC-F2-voice-tts-04`: dokładność prefiksu ±1 słowo ≥ 90% (z `voice-dialog`).

## Fake
`voice-tts-fake`: zwraca WAV z fixture'ów z adnotowanymi znacznikami słów, sterowane TTFB/RTF z wirtualnym zegarem, symulowane błędy silnika (fallback).

## Otwarte pytania
- Wybór modelu PL dla Pocket TTS i licencje głosów v0 (Pocket/Piper) — ADR (11).
- Forced alignment: własny (whisper.cpp) czy aligner z silnika — do ustalenia w SPEC v1.

## Decyzje v0 (F2)
- Pocket TTS PL: trwały sidecar z protokołem JSON-lines po stdio (opis w `crates/voice-tts-impl/README.md`); Piper: proces na zdanie (`--output_raw`).
- Głosy v0: mówczynie `pl-f1`/`pl-f2` (Pocket) × wysokość/tempo (Alfa 1,00/1,00, Beta 1,06/0,97, Gama 0,92/0,92, Delta 1,12/1,08); zapas Piper `pl_PL-gosia-medium` z odpowiadającą wysokością. Modyfikacja: WSOLA + resampling (błąd F0 i długości < 0,01% w testach, próg ±3%). Nazwy głosów i licencje — casting (ADR 11).
- Fallback per zdanie (łańcuch idzie dalej, nie wraca w obrębie wypowiedzi); w sesji prywatnej ogniwa chmurowe pomijane.
- Znaczniki: natywne z silnika (przeskalowane o tempo presetu) albo estymata z długości słów (`Estimated`); forced alignment — SPEC v1.
