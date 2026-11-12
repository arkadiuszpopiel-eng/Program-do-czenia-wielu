# Surowe dowody diagnozy CPU — 12 listopada 2026

[Raport i ocena odpowiedzi](../../2026-11-12-cpu-extended-models.md) ·
[Archiwum 165 plików](raw-evidence.tar.gz) · [Manifest SHA256](manifest.json).

SHA256 archiwum: `7397e4d1b2675b2273c8f5fe98d936588160cadcd61be483792c56506694810b`.
Wagi modeli nie są w repozytorium; publiczne rewizje, pełne hashe i wyniki
weryfikacji są w plikach cpu-diag/main-model-entry/actual-read-pins.

## Zakres

Bazowe źródło: `7112cfe1058e89f1668f8895293c751578181295` (PR86).
Opcje ręcznej diagnozy: ba6bf7fa; dokumentacja scalona na29d23510.
Receipty wskazują również źródła niecommitowane. Migawki cpu8-source, cpu11-source,
cpu14-source, cpu-grounding-source i cpu-final-inference-source zachowują użyte
wersje adapterów; końcowe cztery źródła mają dodatkowo hashe w manifest.json.
Pozostałe pliki fixture pochodzą z bazowego commitu. Skrypty kontrolerów ewoluowały;
komenda i pełne żądanie konkretnego uruchomienia są wiążącym zapisem jego profilu.

CPU, Delta/Standard, rzeczywisty LocalProvider i natywny odczyt własnego TEMP.
Pierwszy wybór narzędzia skryptowany; następna odpowiedź prawdziwego modelu.
Negatywna kontrola ma inną sesję bez działań i dawną fałszywą deklarację asystentki.
Nie jest to test GPU, autonomicznego tool calling, kodowania, wiedzy ogólnej,
telefonu, mikrofonu ani prawdziwego API. Nie ma sekretów lub danych właściciela.

## Zawartość i interpretacja

- `cpu-final-semantic-review.json`: ręczna ocena wszystkich11 uruchomień:
  10 odpowiedzi (3 poprawne w danej kontroli,7 błędnych),1 FirstToken timeout.
  Różne profile i adaptacyjny wybór hipotez: nie używać jako benchmarku skuteczności.
- `cpu-extended-*`, `cpu-grounding-*`, `cpu-system-*`: receipty, logi, końcowe żądania,
  teksty, pomiary, hipotezy zadeklarowane przed próbami i oceny.
  `exit0` potwierdza transport/wejście, nie prawdziwość odpowiedzi.
- `cpu-docs-*`: wszystkie polecenia zakończone exit0. Prettier pomija `docs` przez `.prettierignore`, więc ten wynik **nie poświadcza formatowania Markdown**. Generator katalogu, HTML i weryfikacja HTML wykonane osobno.
- `cpu-final-format`, `cpu-final-controls`, `cpu-final-clippy`: końcowe0;
  kontrole3/0/4ignored, bez uruchamiania pominiętych testów wielopersonowych.
- `cpu-reasoning-build` i `cpu-grounding-build`: niezaliczone kompilacje101,
  poprawione buildy osobno. To nie behawioralny RED i nie wyniki modeli.
- `cpu-diag-*`, `resume_cpu11.py`: publiczne piny, przerwany transfer11B900s,
  wznowienie z walidacją zakresów i pełnym SHA; cache11B usunięty po zakończeniu
  jego testów, przed pobraniem14B. Zachowano protokół usunięcia.
- `cpu11-memory-sample`, `cpu-final-negative-memory-sample`: punktowy RAM CPU,
  granica cgroup16GiB, bez OOM/kill; brak pomiaru VRAM. Opcjonalne memory.pressure
  niedostępne, nie zastąpiono go wymyśloną wartością.
- `cpu-final-jobs-*` i `cpu-final-joblog-*`: pełne stany/logi ukończonych CI194–197.
  Każdy10/10; Windows Rust3669/0/49,3679/0/49,3680/0/50,3680/0/50.
  CI197 Linux3658/0/29,Vitest593,Chromium321+321+321=963.
  To wyniki **poprzednich headów**, nie nowej gałęzi tej diagnozy.
- `windows121-live/*.json` i logi decoded: mocniejszy odczyt Windows poprawny
  transportowo, ale błędne zaprzeczenie8B. Artefakt13924082802 z workflow43173642177,
  job131032589662; oryginalny ZIP94729B i jego SHA ponownie sprawdzone.
  WebView2 na7112cfe1:63/0/5, cały Windows1213/4failure.

Żaden wariant nie uzyskał kwalifikacji.14B bazowo i ze stałą regułą:1/2.
Model domyślny, produkcyjny prompt i zamrożone progi bez zmian. #58/#77 otwarte.
