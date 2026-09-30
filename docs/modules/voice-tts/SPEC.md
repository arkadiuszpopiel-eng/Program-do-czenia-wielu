# voice-tts — SPEC (szkic v0)

## Cel
Synteza mowy jako osobny proces: lokalnie na baseline Pocket TTS + model PL społeczności (CPU, RTF ≈ 0,21, ~200 ms do 1. fragmentu, klon z krótkiej referencji), Piper pl_PL jako zapas; poza baseline (profil D-CUDA) Chatterbox/XTTS-v2/F5; chmura (ElevenLabs, Cartesia, Google, Azure, Gemini, OpenAI, MiniMax) przez `ModelProvider`. Chunker, streaming zdanie-po-zdaniu, dostawca znaczników słów, cache fraz stałych, łańcuch fallback per agentka (PLAN §6.2, §6.3, §6.6–6.7).

## Fala i priorytet
F0: spike (e) tabela kandydatów; F2: moduł z głosami v0 (≥ 2 bazowe mówczynie + wysokość/tempo, bez kluczy); casting właściwy po kluczu. P0.

## Kontrakt (szkic Rust)
```rust
// voice-tts-contract — SZKIC
pub struct VoiceRef { pub persona: PersonaId, pub engine: TtsEngine /* Pocket | Piper | Chatterbox | Xtts | Cloud(AccountId, VoiceId) */,
                      pub preset: VoicePreset /* v0: base_speaker, pitch, rate */, pub reference: Option<PathBuf> /* klon po castingu */ }
pub struct TtsChunk { pub audio: Frame, pub word_marks: Vec<WordMark { word_idx, t_start, t_end }>, pub marks_kind: MarksKind /* Native | ForcedAlign | Estimated */ }
pub struct TtsRequest { pub text: String /* po normalizatorze PL, ze znacznikami stylu */, pub voice: VoiceRef, pub style: StyleTags, pub utterance: UtteranceId }
pub trait Tts: Send + Sync {
    fn voices(&self) -> Vec<VoiceInfo>;
    fn synth(&self, req: TtsRequest, cancel: CancelToken) -> BoxStream<TtsChunk>;      // streaming
    fn stop(&self, utterance: UtteranceId);                                           // ≤ 20 ms
    fn warm(&self, voice: &VoiceRef) -> Result<()>;                                    // preload + cache fraz
    fn health(&self) -> Health;
}
```
Zdarzenia: `tts.started` (TTFB), `tts.chunk` (Diagnostics), `tts.finished`, `tts.stopped`, `tts.fallback` (silnik zapasowy, per agentka), `tts.cloud.sent`.

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
`process` (JSON-RPC po pipe), `on-demand` z `idle_unload`; 24–48 kHz wyjście.

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
