# Surowe dowody — fakty czynności i własne persony, 12.11.2026

[Pełny raport](../../2026-11-12-execution-records-personas.md) · [oryginalne dane](raw-evidence.tar.gz) ·
[manifest plików](manifest.json) · [metadane archiwum](archive.json).

Archiwum: **302 plików**, 58708942 bajtów przed kompresją,
17529243 bajtów po kompresji. SHA256:
`9c9836fa74b770f6321e5659d78f269f3105dd73df4df4a08767f5cc99565438`.

Każdy wpis manifestu ma rozmiar i SHA256 oryginalnych bajtów. Logi nie są odtwarzane
z transkrypcji ani normalizowane. W środku są osobne archiwa test-results z każdej
próby, w tym porażki i pomyślne powtórzenie; zawierają zrzuty strony/PNG, gdy zapisał je test.
Żądania dotyczą syntetycznych fixture'ów, bez rzeczywistych sesji właściciela, kluczy,
wag modeli lub zależności binarnych. Auth lokalnej próby jest jawnie zredagowany w statusie.

## Jak sprawdzić wynik

1. Sprawdź hash archiwum, rozpakuj do nowego pustego katalogu i porównaj każdy wpis z manifest.json.
2. `rust-batches-verification.json` oraz `rust-batches.json`:3628/0/27,71/71 etapów,
   884/884 binariów i264/264 doctestów. `rust-ignored-tests.json` ma27 pominięć i powody.
   `workspace-tests-records.log` zachowuje wcześniejszy błąd dysku/linkera przed testami.
3. `ui-coverage-verification.json`:961 różnych ID; połączono ui-shard-1 z ui-resume-shard-2/3/4.
   ui-shards.json i ui-shard-2 zachowują pierwotne345/1/615. Pięć izolowanych prób oraz
   powtórzony shard2 nie powiększają liczby unikalnych przypadków. Przyczyna opóźnienia niepotwierdzona.
4. `records-*.json` i logi mają komendy, source_head, source_status, czas i kod wyjścia.
   Test wire wykonano przed commitem na tych samych bajtach późniejszego c2; Clippy/deny na czystym c2.
5. `core-captured-records/` ma trzy żądania z Core, `core-codec-bodies/`60 ciał z produkcyjnego
   kodeka (native/schema). To atrapy odpowiedzi, bez prawdziwego serwera lub modelu.
6. `pilot-records_user_data/` i `pilot-records_ablation/`: po1timeout pierwszej generacji,
   0tekstów i2niewykonane, NOT_EVALUATED; rzeczywisty Qwen8B CPU, bez konkurujących obciążeń.

Samodzielny `verify_ui_coverage.py` w rozpakowanym katalogu sprawdza kompletność,
niepokrywające się ID, brak retry/pominięć i hashe logów/reportów/archiwów każdej części.
Skrypty runnerów są zapisem metody; ich ponowne wykonanie wymaga przypiętego środowiska
i zgodnego checkoutu. Nie uruchamiać ich równolegle lub traktować jako harmonogram nocny.
Kody powłoki nadrzędnej nie zastępują pola exit we właściwym JSON.

To **Linux/headless Chromium/fake IPC**. Wynik nie obejmuje Windows/WebView2,
fizycznego telefonu, rzeczywistego API, RTX2060 12GB lub mikrofonu.

## Końcowe kontrole dokumentacji i repo

Po zapakowaniu surowych prób uruchomiono generatory katalogu/funkcji/HTML/planu,
HTML verify, Rustfmt, IPC, wersję, lint i diff: wszystkie kod0. Oryginalne logi i
komendy są osobno w validation/; [ich manifest](validation-manifest.json) nie jest
częścią wcześniejszego archiwum i nie zmienia jego hasha.

[Atlas desktop — e958](atlas-desktop-e958.png) ·
[Obsada Atlasu320px po przewinięciu — 894](persona-atlas-320.png) ·
[Adresatka320px — 894](persona-recipient-320.png).
