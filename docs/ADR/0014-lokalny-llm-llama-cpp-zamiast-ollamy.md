# ADR 0014 — Lokalny LLM: wbudowany llama.cpp (Vulkan/CUDA/CPU) zamiast Ollamy

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany (próg jakości tool-use PL na 3–4,5B mierzony w F3; tok/s — spike (h)) |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §1.2 (Lokalny LLM), §3.4, §3.5, §5.2 (klasa A), §5.4, §5.6, §6.3, §14.5, §16.2 (F1, F3, F6), §17 |

## Kontekst

Na starcie nie ma kluczy API ani kont. Program musi działać bez nich: potrzebny lokalny „mózg" do komend, fallbacku i trybu offline, pobierany w onboardingu. Baseline ma AMD RX 7600 8 GB (Vulkan, bez CUDA), z czego pulpit zajmuje 0,5–1 GB, a STT 1–2,5 GB. Laptop ma tylko 6 GB VRAM. Ollama opakowuje llama.cpp z opóźnieniem i jako osobny serwis.

## Decyzja

| Element | Wybór |
|---|---|
| Silnik | **llama.cpp** wbudowany jako sidecar (`providers-local`), backendy Vulkan (AMD), CUDA (NVIDIA), CPU (fallback) |
| Modele domyślne | 3–4,5B Q4_K_M (np. Bielik 4.5B), pobierane w onboardingu, gdy nie ma kluczy; format GGUF z hashem |
| Kwanty | **bez kwantów IQ** (crashe na RDNA3/Vulkan) |
| 8B | tylko gdy STT idzie na CPU (budżet VRAM 8 GB); desktop 16 GB: 8–14B Q4 |
| Rola | **bez kluczy — domyślny i jedyny „mózg" (MVP)**; z kluczami — komendy, fallback, offline; zaawansowana rozmowa domyślnie przez API |
| Zewnętrzne lokalne | Ollama i LM Studio jako **zewnętrzne endpointy** przez adapter generyczny (`ModelProvider`), nie jako wymóg |
| Rezydencja | `model-residency`: STT+TTS+LLM nie zawsze naraz; wykrywanie pełnego ekranu/gier → LLM na CPU lub chmurę |

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Ollama jako wymagany backend | opóźniony względem llama.cpp; osobny serwis i instalator; mniej kontroli nad VRAM i kwantami; nadal dostępna jako zewnętrzny endpoint |
| LM Studio | GUI-first, zamknięte; jak wyżej — tylko endpoint |
| Modele 7–8B jako domyślne na baseline | nie mieszczą się obok STT w 8 GB VRAM; na laptopie 6 GB tym bardziej |
| Brak lokalnego LLM w MVP | bez kluczy program nie miałby „mózgu"; decyzja właściciela: klucze później |
| Kwanty IQ dla oszczędności VRAM | zgłoszone crashe na RDNA3/Vulkan |
| ONNX Runtime GenAI / DirectML dla LLM | DirectML w maintenance; llama.cpp ma dojrzalsze Vulkan/CUDA |

## Konsekwencje

- `providers-local` wymagany w MVP (F1): lokalny llama.cpp na żywo w kryteriach F1; eval narzędzi fs/shell na lokalnym modelu ≥ próg ustalony w F0 (F3).
- Uczciwie: tool-use po polsku na 3–4,5B jest ryzykiem; computer use (F6) **wymaga klucza API lub mostu** — lokalny model nie osiągnie progu.
- Onboarding: pobieranie modelu z wznawianiem, komunikat o jakości rozmowy ograniczonej rozmiarem LLM (profil A).
- Spike (h): tok/s modeli 4B/8B pod limitami baseline (korekta GPU ×2,2 z desktopu), 0 crashy w 1 h.
- Router traktuje lokalny model jako klasę A z tagiem `local_only` — sesje „prywatne" mogą wymusić go jako jedyny.
- Przypięta wersja llama.cpp + sterownik Adrenalin; `docs/vendor/llama-cpp.md`.

## Jak cofnąć

- Ollama/LM Studio już działają przez adapter generyczny; uczynienie ich domyślnymi to zmiana konfiguracji `device-profile`, nie kodu.
- Wymiana silnika (np. na inny runtime GGUF) to nowy `-impl` za `providers-local-contract`.
- Usunięcie lokalnego LLM z MVP wymagałoby zmiany decyzji właściciela o działaniu bez kluczy.
