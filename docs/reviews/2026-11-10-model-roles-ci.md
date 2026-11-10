# RozliczenieCI177/Windows99 i korekta próbyCPU

Head3e972aa0: CI177 zakończył się porażką dwóch z10 zadań. Clippy naLinux iWindows odrzucił
komentarz app-models/lib.rs (doc_lazy_continuation); pełne testyRust nie rozpoczęły się.
Pozostałe8 zadań zielone:Chromium915/915 (305+305+305,8,8/9,8/8,4min),Vitest573/573,
Tauri,cargo-deny,dokumentacja. Windows99:3/4 zielone;WebView263/0/5 w2,8min,
setup i adresy/hasheHF zaliczone; próba rzeczywistych silnikówCPU niezaliczona.
Żadnego przebiegu nie anulowano kolejnym pushem.

## Rzeczywisty wynikCPU — bez odbioruGPU lubCore

Qwen3-8B Q4_K_M:4 wątki, start13951ms,pierwszy token15762ms,96 tokenów w30296ms,
6,61tok/s. Oddzielne żądanie zwróciło strukturalną propozycję narzędzia w19128ms;
nie jest to dowód wykonania akcji przez narzędzie/Brokera. To krótki smoke ModelProvider,
nie próba pełnegoCore, wszystkich person, latencji lubRTX2060 12GB.

Bielik11B nie wygenerował tekstu: produkcyjna ochrona odmówiła startu przy8192MiB budżetu
RAMCPU. Nie podniesiono budżetu, limitu czasu lub progów ewaluacji. Nie ogłaszamy generacji
11B zaliczoną. OdbiórGPU12GB pozostaje wymagany.

## Korekta i jej sprawdzenie

Przywrócono poprawny akapit dokumentacji, łącząc dwie linie opisu; plik nadal400 linii.
Clippyall-targets/all-features0. Początkowa lokalna komenda pracowała jeszcze na opublikowanym
worktree i odtworzyła porażkę101; GREEN wykonano jawnie w poprawionym worktree.

Każdy model nadal trafia do próbyCPU. Dla modelu większego niż istniejący budżet test osobno
wymaga dokładnej odmowy pamięci,0 tokenów i braku uruchomionego serwera; raport zachowuje
błąd i status **NOT_RUN_CPU_BUDGET**. Nie jest to zaliczona generacja. Inne błędy nadal
nie zaliczają próby. Generacja modeli mieszczących się w budżecie nadal wymaga≥16 tokenów
i polskich liter; cały przebieg wymaga co najmniej jednej rzeczywistej generacji.

Deterministyczna regresja korzysta z produkcyjnego zarządcy pamięci i kontrolowanego
uruchamiacza bez procesów:11B jest odrzucony, a nieoczekiwana awaria startu8B nie jest
ukryta jako spodziewana odmowa. Początkowa atrapa nie miała pliku serwera; uzupełniono fixture,
log zachowany. Końcowy cel live_catalog:3/0/1, pełne app-models86/0/1,Clippy0.
Ignorowana próba rzeczywistych silników nie została tutaj ponowiona lokalnie.
[Dokładne wyniki i hashe](artifacts/model-roles-ci-correction.json).

Nowy commit wymaga własnego pełnegoCI. Ta korekta nie nadaje uprawnień, nie zmienia
polityk lub zamrożonych progów i nie uznaje nieudanegoCI177/Windows99 za zaliczone.
