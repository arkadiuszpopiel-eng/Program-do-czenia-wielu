# Modele główne8–12B i lekki pomocnik — #65

Nowe zlecenie właściciela po zamknięciu nocnego okna. Qwen3-8B Q4_K_M jest domyślny,
Bielik11B Q4_K_M alternatywą tekstową; oba z oficjalnych repozytoriów, przypiętą rewizją iSHA256.
Bielik1.5B pozostaje pomocnikiem: brak głównego modelu nie promuje go automatycznie.
Qwen dostaje oficjalny parametr szablonu enable_thinking=false, bez zwiększania budżetów.

Przed zmianą1/3 testów RED, osobny RED automatycznej promocji pomocnika0/1. Po zmianie
pełne providers-local80/0/0 (12 wyników celów), Clippy all-targets/all-features0.
Końcowa kontrola rezerw mowy (STT1500+TTS600MiB) i ekranu768MiB:5/5.
Qwen8k:6497MiB wagi/narzut/KV; po mowie/ekranie zostaje2923MiB z12GiB.
Bielik11B8k:8563MiB; pozostaje857MiB, więc8B pozostaje zalecany. Przy16k Bielik musi
zmniejszyć kontekst do8k w tym budżecie. W Qwen poprawiono wcześniejsze dane próbnego wpisu:
rzeczywisty GGUF ma36 warstw i144MiB KV/1k, nie32/128. Oficjalny config Bielika ma50/200.

To rachunek pamięci i testy kontrolowane, nie odbiór RTX2060 12GB lub certyfikat jakości.
Poprzednia porażka CoreCPU Qwen i #41/#48 są zachowane; większy model wymaga własnej ewaluacji.
Bielik11B nie ma potwierdzonego tool calling i deklaruje tools=false. Karty menedżera/pakiety
oraz routing pomocy chmurowej wymagają osobnych kolejnych przyrostów przed publikacją całości.
[Walidacja](artifacts/local-main-models-validation.json). Bez zmian polityk lub progów.
