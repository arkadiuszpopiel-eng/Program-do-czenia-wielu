# Menedżer modeli: główny8B, pomocniczy1.5B — #66

Qwen3-8B Q4_K_M jest pierwszym LLM katalogu, więc onboarding pobiera główny model.
Na osobnej karcie12GB pakiety1–6 zawierają Qwen8B zamiast1.5/4.5B; opisy i uwagi o jakości
pokazują rzeczywisty skład i szacunki pamięci. Pozostałe składniki głosu pozostają zależne
od pakietu. Karty nowego8B i11B oraz ich licencji są kompletne, bez potwierdzenia człowieka.
Karta1.5B mówi wprost o funkcji pomocniczej i niezaliczonych faktach/wykonaniu działań.

Przed zmianą0/3 testów nowego profilu. Pierwszy pełny build zakończył linker101/SIGBUS przy
braku miejsca wCloud; zachowano błąd. Po uporządkowaniu nieużywanego cache pełne app-models:
85 zaliczonych/0 błędów/1 ignorowany,17 wyników celów. Clippy all-targets/all-features kod0.
Ignorowany test live_catalog wymaga rzeczywistych silników/modeli; nie zaliczamy go tutaj.

Binaria wcześniejszych prób modeli zachowano w gzip z SHA256 sprawdzonym po dekompresji,
bez usuwania ich danych lub logów: /workspace/scratch/model-roles/preserved-probes/manifest.json.
Zmiana nie pobiera ani nie uruchamia modelu automatycznie. To profile do odbioru na RTX2060
12GB, nie pomiarGPU lub jakości większych modeli. Profile pakietów słabszego sprzętu są
historyczne; nie omijają blokady automatycznej promocji małego modelu do głównej trasy z#65.
[Walidacja](artifacts/main-model-profile-validation.json).
