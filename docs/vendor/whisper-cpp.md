# whisper.cpp — sidecar `whisper-server` (`voice-stt-impl`)

- Wersja: whisper.cpp **≥ 1.8.1** (przypięta przy budowie sidecarów; buildy Vulkan / CUDA / CPU osobno).
  API zweryfikowane na źródle `examples/server/server.cpp` (whisper.cpp w sdist `pywhispercpp` 1.5.1), 2026-10-01.
- Model: `ggml-large-v3-turbo-q5_0.bin` (547 MiB), hash sprawdza instalator modeli; poza repo.

## Uruchomienie
`whisper-server -m <model> --host 127.0.0.1 --port <wolny> -t <wątki> -nlp -sns [-fa | -ng]`
(`-ng` = CPU, `-fa` = flash attention na GPU, `-nlp` = bez drogiego przebiegu prawdopodobieństw języków,
`-sns` = tłumienie tokenów nie-mowy; dostępne też `--vad -vm <silero ggml>` — u nas bramka VAD jest przed wysłaniem).

## HTTP
- `GET /health` → 200 `{"status":"ok"}` albo 503 `{"status":"loading model"}`.
- `POST /inference` (multipart): `file` (WAV), `response_format=verbose_json`, `language` (`auto`/`pl`/`en`),
  `temperature`, `beam_size`, `best_of`, `prompt` (hotwords), `token_timestamps=true`, `suppress_non_speech`,
  `no_timestamps`, `vad`… → `{"task","language":"polish","duration","text","segments":[{"id","text","start","end",
  "tokens":[…],"words":[{"word":" Del","start","end","t_dtw","probability"}],"temperature","avg_logprob","no_speech_prob"}]}`.
  `words` to **tokeny** (podsłowa): token zaczynający się spacją otwiera słowo; tokeny `[_BEG_]` itp. pomijamy.
  Język jako pełna nazwa (`polish`) → mapujemy na ISO.
- `POST /load` (`model=<ścieżka>`) — podmiana modelu bez restartu (nieużywane w v0).

## Awarie
`ErrorDeviceLost` / `VK_ERROR_DEVICE_LOST` / `CUDA error` w stderr albo wyjście procesu w trakcie żądania →
fallback CPU (`-ng`), ponowienie wypowiedzi z bufora; przypięty sterownik Adrenalin (ryzyko RDNA, ADR 0004).
