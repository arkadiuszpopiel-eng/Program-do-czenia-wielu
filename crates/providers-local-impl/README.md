# providers-local-impl

Lokalny `ModelProvider` (docs/modules/providers-local/SPEC.md, ADR 0014): llama.cpp jako proces-sidecar
`llama-server`, manifest `module.toml` (`providers-local`, `process`, `on-demand`). Kontrakt: `providers-contract`.

- **Sidecar** (`Sidecar`): start na żądanie (jeden naraz), `--host 127.0.0.1`, losowy port i losowy
  `--api-key` (192 bity) per uruchomienie, zdrowie `GET /health`, zwolnienie po bezczynności
  (`idle_unload`, domyślnie 10 min), restart po awarii z limitem (`max_restarts` w `restart_window`),
  fallback GPU → CPU (`-ngl 0`) ze zdarzeniem, dzierżawa `model-residency` (eksmisja → wyładowanie,
  tryb gry → przeładowanie na CPU), `kill_on_drop`, bez okna konsoli na Windows, stderr → logi z redakcją klucza.
- **Argumenty z profilu urządzenia** (`LocalConfig`, `LaunchPlan`): backend Vulkan/CUDA/CPU (osobne pliki
  `llama-server` per backend; bateria/słaby sprzęt → CPU), `-ngl` wg budżetu VRAM po rezerwie na STT na GPU
  (pełne odciążenie albo tyle warstw, ile mieści się po stałym narzucie 550 MB), `-c` (na GPU zmniejszany do
  4096 tylko wtedy, gdy pozwala to zmieścić model w całości), `--threads` (rdzenie fizyczne), `--alias`, `-np 1`,
  `--jinja` (modele z narzędziami).
- **Pamięć**: `vram_mb`/`ram_mb` z manifestu bez KV cache + `kv_mb_per_1k_ctx` × kontekst; dzierżawa
  `model-residency` liczy VRAM i RAM dla liczby warstw na karcie (`ModelEntry::vram_for`/`ram_for`), w trakcie
  żądania jest oznaczona jako używana i odświeżana po nim (bez zwalniania co 10 min w trakcie rozmowy).
  Laptop RTX 4050 6 GB: Bielik 4.5B Q8_0 — 34/60 warstw (3570 MB) + whisper CUDA (1500 MB); 1.5B — w całości.
- **Strumień**: wspólny silnik `lib-openai-compat` (Chat Completions); `Started` od razu po przyjęciu
  żądania (zimny start modelu nie jest „milczeniem" dla Routera); koszt 0, tokeny raportowane; sesje
  prywatne i dowolna jurysdykcja dozwolone (dane nie opuszczają maszyny).
- **Modele** (`models.toml`, tylko oficjalne repozytoria speakleash): Bielik 4.5B v3.0 Instruct Q8_0 (domyślny,
  4,8 GB) i Bielik 1.5B v3.0 Instruct Q8_0 (lekki, 1,6 GB, bez narzędzi); walidacja: tylko `.gguf`, **bez kwantów IQ**,
  https, SHA-256 64 hex albo pusty. **Pobieranie** (`Downloader`): wznawianie HTTP Range z `.part`
  (do 3 automatycznych wznowień), SHA-256, zły hash → błąd i usunięcie `.part`; pusty hash w manifeście →
  zapis hasha przy pierwszym pobraniu (`<plik>.sha256`) z ostrzeżeniem w logach; katalog wstrzykiwany
  (`%LOCALAPPDATA%\Alfa\models`). Zdarzenia `local.model.download.*`, `local.model.loaded/unloaded`,
  `local.backend.fallback`, `local.sidecar.crashed`.

Testy (bez modelu, GPU i internetu): binarny cel `fake-llama-server` (tylko testy; std + serde_json) —
pełny zestaw kontraktowy `ModelProvider` na prawdziwym procesie, cykl życia (start na żądanie, port/klucz,
restart po awarii z limitem, fallback GPU → CPU ≤ 5 s, dzierżawy, bezczynność, zdarzenia), anulowanie
≤ 100 ms; pobieranie z przerwaniem i wznowieniem, zły hash, pierwszy hash, anulowanie (lokalny serwer HTTP);
profil laptopa właściciela (`tests/laptop.rs`, `tests/laptop_voice.rs` — kolejne tury głosowe bez przeładowań).
