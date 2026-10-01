# voice-tts-impl

Implementacja `voice-tts`.

- **Pocket TTS PL** — trwały sidecar, protokół JSON-lines po stdio (jedno żądanie naraz):
  - żądanie: `{"id":1,"op":"synth","text":"…","voice":"pl-f1","energy":1.0,"emotion":null}`
  - odpowiedzi (dowolnie wiele, inne linie ignorowane): `{"type":"audio","id":1,"pcm16":"<base64 PCM16 LE>","sample_rate":24000}`,
    `{"type":"word","id":1,"word":"…","start_ms":0,"end_ms":240}` (opcjonalnie), na końcu `{"type":"done","id":1}`
    albo `{"type":"error","id":1,"message":"…"}`.
  Wrapper sidecara (Pocket TTS bez Pythona) budowany osobno w `sidecars/`; awaria procesu → ponowne uruchomienie.
- **Piper** `pl_PL-*` — proces na zdanie: `piper --model <dir>/<głos>.onnx --output_raw --quiet`, tekst na stdin,
  surowe PCM16 na stdout, częstotliwość z `<głos>.onnx.json` (`audio.sample_rate`, domyślnie 22 050).
- **Głosy v0** (`voice_mod`): tempo **WSOLA** (okna Hanna 20 ms, zakładka 50%, dopasowanie ±10 ms), wysokość =
  WSOLA + resampling sinc. Dokładność w testach: F0 ×0,88…×1,2 i tempo ×0,8…×1,25 — błąd < 0,01% (próg ±3%).
- `TtsService`: chunker → łańcuch per agentka (fallback per zdanie, zdarzenie raz, chmura pomijana w sesji prywatnej,
  adapter chmurowy — kolejna fala) → preset → 24 kHz → znaczniki natywne (przeskalowane o tempo) albo estymowane →
  TTFB (`started`), `stop` anuluje, **cache fraz** (`PhraseCache`: SHA-256 tekst+brzmienie+tempo, ≤ 50 MB, LRU).
- `TtsLease` — dzierżawa `model-residency` (CPU). Zależności: `sha2` 0.10.9, `base64` 0.22.1.

Do weryfikacji na sprzęcie/z modelami: TTFB p50/p95 (ACC-F2-voice-tts-02), odrębność ECAPA/ABX (-03), prefiks ±1
słowo (-04), ślepa ocena (F0 spike e). Modele i binaria nie są w repo (HuggingFace/GitHub zablokowane w CI).
