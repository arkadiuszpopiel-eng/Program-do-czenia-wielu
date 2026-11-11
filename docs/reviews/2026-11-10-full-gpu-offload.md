# Pełne odciążenie modelu w llama.cpp — #79

Kod: `54dfabf7266b31855e91670653dac08201cc3835`, baza `bb13c48d`.
Naprawa lokalnego dostawcy; odbiór fizycznego GPU pozostaje niewykonany.

## Przyczyna i zmiana

Przypięty [llama.cpp b11559](https://github.com/ggml-org/llama.cpp/blob/b11559/src/llama-model.cpp)
w `n_gpu_layers()` dolicza poziom wyjściowy poza blokami modelu. Początek mapowania GPU w
`load_tensors()` wynosi `max(n_layer_all + 1 - n_gpu_layers, 0)`. Aplikacja przekazywała tylko
liczbę bloków, choć plan i rachunek RAM/VRAM zakładały pełne odciążenie. Na tej wersji
pozostawiało to jeden blok na CPU. To dowód z kodu zależności, bez pomiaru CUDA/Vulkan.

`LaunchPlan` nadal przechowuje liczbę bloków dla planowania pamięci. Przy składaniu argumentów
pełne odciążenie dolicza dodatkowy poziom: Qwen8B 36→37, Bielik11B 50→51, pomocniczy1.5B 32→33.
Częściowy limit i CPU0 pozostają dosłowne. Budżety pamięci, rezerwy, kontekst i wybór modeli
nie zostały zwiększone. Szacunki VRAM z manifestu nadal wymagają pomiaru na docelowej karcie.

## Weryfikacja

- RED: 1 zaliczony / 1 niezaliczony; odtworzono Qwen8B `-ngl36`, wymagane37.
- Pierwszy pełny przebieg po poprawce: exit101, stara asercja pełnego odciążenia oczekiwała32
  zamiast33 w teście przejścia GPU→CPU; zachowano log, skorygowano tę samą asercję kontraktu.
- Końcowy `cargo +1.94.1 test --locked -p providers-local-impl --all-features -j2`:
  **91 zaliczonych / 0 błędów / 1 pominięty**, 17 podsumowań celów testowych.
- Clippy wszystkich celów/cech z `-D warnings` i formatowanie: exit0.
- Pierwszy lint: exit1, w nowym worktree brakowało `node_modules`. Po podłączeniu istniejących,
  przypiętych zależności, bez zmiany lockfile: lint exit0, zero naruszeń rozmiaru/Svelte/CSS.
- Dwie nowe regresje obejmują pełny start8B/11B/1.5B dla CUDA i Vulkan oraz dosłowne limity
  częściowe i CPU. Istniejący test procesu sprawdza także `-ngl33` i przejście do CPU0.

[Komendy, kody wyjścia, SHA256 i wycinki przypiętego źródła](artifacts/full-gpu-offload-validation.json).
Nie jest to wynik sprzętowy RTX2060 12GB ani dowód poprawy szybkości. Nie zmieniano polityk,
progów evals, chronionego Jądra/Brokera ani zależności. PR24 pozostaje draft do niezależnego przeglądu.
