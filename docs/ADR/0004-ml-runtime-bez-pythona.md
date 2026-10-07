# ADR 0004 — ML runtime bez Pythona (whisper.cpp Vulkan/CUDA, ONNX Runtime CPU, sherpa-onnx)

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany (wartości liczbowe do potwierdzenia spike'ami (e) i (h) w F0) |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §1.2 (ML), §3.4, §3.5, §6.2, §6.3, §8.7 (łańcuch dostaw), §16.2 (F0), §17 |

## Kontekst

Baseline to Ryzen 5 5600 + **Radeon RX 7600 8 GB (AMD, bez CUDA)** + 16 GB RAM. Program ma być lekki (instalacja ≤ 40 MB bez modeli), a modele głosowe muszą działać lokalnie w profilu A bez kluczy. Runtime Pythona z PyTorch to setki MB i niestabilny ROCm na Windows dla RDNA (gfx110X). DirectML jest w trybie maintenance.

## Decyzja

| Zadanie | Runtime | Uwagi |
|---|---|---|
| STT | **whisper.cpp**, backend **Vulkan** pod AMD, CUDA na NVIDIA, fallback CPU (`-ng`) | wersja przypięta (min. 1.8.1); modele large-v3-turbo Q5_0 (547 MiB) z bramką VAD; large-v3 pełny na desktopie 16 GB |
| VAD, koniec tury, KWS, embedding mówcy | **ONNX Runtime na CPU**, sherpa-onnx | małe modele (Silero VAD, Smart Turn v3.2 ~8 MB, WeSpeaker/ECAPA) — CPU wystarcza |
| TTS lokalny (baseline) | Pocket TTS + model PL społeczności (CPU, RTF ≈ 0,21), Piper pl_PL jako zapas | szczegóły w ADR 11 |
| LLM lokalny | llama.cpp Vulkan/CUDA | ADR 14 |
| Python | **tylko opcjonalny „Model Pack"** | poza baseline: Chatterbox Multilingual, XTTS-v2, F5 na maszynach z NVIDIA/CUDA (profil D) |

Zasady: formaty modeli tylko safetensors/ONNX/GGUF z hashami; wersje bibliotek i sterownik Adrenalin przypięte; automatyczny fallback na CPU po `ErrorDeviceLost`; `model-residency` pilnuje VRAM (pulpit 0,5–1 GB + STT 1–2,5 GB + LLM 3–4B obok STT; 8B tylko przy STT na CPU).

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| PyTorch + ROCm na Windows | niestabilny dla gfx110X; RTF ≥ 1 dla Chatterbox/XTTS/F5 na baseline; ciężki runtime |
| DirectML jako wspólny backend GPU | tryb maintenance; małe modele i tak działają na CPU |
| Ollama zamiast llama.cpp | opóźniony względem llama.cpp; osobny serwis — ADR 14 |
| faster-whisper (CTranslate2) | wymaga Pythona lub własnych bindingów; brak stabilnego Vulkana pod AMD |
| Parakeet v3 (ONNX) jako główny STT | szybki na CPU, ale gorszy WER PL niż Whisper large-v3 (FLEURS-PL ~7,3% vs 4,7% wg źródła wtórnego) — zostaje kandydatem w Voice Lab |
| Wyłącznie chmura dla STT/TTS | profil A musi działać bez kluczy; prywatność audio |

## Konsekwencje

- Sidecary (`sidecars/`) dla whisper.cpp i TTS jako osobne procesy (JSON-RPC po stdio/named pipe); ładowane przy pierwszym użyciu, zwalniane po bezczynności.
- Dwie ścieżki GPU w macierzy testów: Vulkan (desktop RDNA4) i CUDA (laptop RTX 4050); RDNA3 (baseline) niepokryty fizycznie — czasy z desktopu × ~2,2 przy ocenie kryteriów baseline; CPU jako górna granica.
- Ryzyko Vulkan/AMD: zgłoszone crashe na RDNA1/3/4 → przypięte wersje, fallback CPU, bramka VAD (turbo halucynuje na szumie). Kryterium spike (h): 0 crashy w 1 h ciągłej pracy, VRAM w budżecie.
- Modele CUDA dostępne jako opcjonalne moduły; nie wchodzą do builda bazowego.
- `docs/vendor/<crate>.md` dla bindingów whisper/ONNX — modele AI nie znają najnowszych API.

## Jak cofnąć

- Dodanie opcjonalnego Model Packu z Pythonem jest przewidziane; nie zmienia jądra ani kontraktów `voice-stt`/`voice-tts` (osobny `-impl`).
- Zmiana backendu STT (np. na Parakeet lub chmurę jako domyślną) to wymiana `-impl` za tym samym kontraktem; wynik spike'u (e) może przestawić domyślne w `device-profile`.
- Decyzja odwracalna do końca F0.
