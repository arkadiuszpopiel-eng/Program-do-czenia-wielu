# voice-stt-impl

Implementacja `voice-stt`: **sidecar `whisper-server` (whisper.cpp ≥ 1.8.1, przypięty)**.

- Uruchamiany na żądanie (`ensure_server`): binarium per backend (`SidecarBinaries`: Vulkan/CUDA/CPU), argumenty
  `-m <model> --host 127.0.0.1 --port <losowy> -t N -nlp -sns` + `-fa` (GPU) / `-ng` (CPU); czekanie na `/health`
  (503 = ładowanie modelu); `ProcessLauncher` (bez okna konsoli, `kill_on_drop`, ogon stderr → `ErrorDeviceLost`).
- Żądania `POST /inference` (multipart: WAV PCM16 16 kHz, `response_format=verbose_json`, `language`,
  `beam_size` 1 dla partiala / 5 dla finala, `prompt` z hotwords, `token_timestamps`, `suppress_non_speech`);
  parser łączy tokeny w słowa, pewność = średnia prawdopodobieństw, język z nazwy pełnej → ISO.
- Bramka VAD (mowa < `min_speech_ms` → pusty final, bez wywołania), dwa przebiegi, **fallback CPU** po awarii
  procesu GPU (wypowiedź ponawiana z bufora, backend oznaczony jako niesprawny, `Health::Degraded`), limit restartów.
- Dzierżawa `model-residency` (rola STT, VRAM 1,5 GB / RAM CPU 1,5 GB); brak VRAM → CPU; odebranie → zamknięcie.
- Klient HTTP: `reqwest` 0.12.28 bez TLS, bez proxy, feature `multipart`.

Testy (`tests/stt.rs`) na udawanym serwerze HTTP w procesie: kontrakt, pola żądań, start z 503, awaria Vulkan →
CPU bez utraty wypowiedzi, rezydencja. Prawdziwy whisper.cpp (WER PL, 1 h na Vulkan/CUDA) — self-hosted.
Job Object: `ProcessLauncher` używa `tokio::process` (potrzebny stderr); izolację Job Object daje alternatywny
launcher na `platform-contract::ProcessPort` (do dodania przy integracji z `platform-windows-impl`).
