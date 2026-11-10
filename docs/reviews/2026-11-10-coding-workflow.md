# Kodowanie asystentek — poprawki i dowody 10.11.2026

Nowe zlecenie właściciela po zamknięciu harmonogramów nocnych. Praca na Cloud Linux,
bez subagentów, mostów CLI, odczytu tokenów lub zmian polityk/progów. PR24 pozostaje draft.
[Plan](../plan/55-enterprise-lokalnie-chmura-i-wyszukiwanie.md#kodowanie-10-11).

## Co poprawiono

### #71 — model tracił końcowy błąd kompilatora

`dev_test` zachowywał końcowe30tys.znaków, a runtime ponownie brał tylko początkowe24tys.
Końcowy błąd typu znikał. Regresja z logiem ponad30tys.znaków odtworzyła problem:
**0 zaliczonych, 1 niezaliczony**. Nowa obserwacja zachowuje podsumowanie i końcówkę,
z jawnym pominięciem środka w **tym samym limicie24tys.znaków**. Krótkie wyniki pozostają
identyczne. Zabezpieczenia niezaufanej treści, obrazy i status narzędzia pozostają zachowane.

Testy Unicode i budżetów obejmują pusty i krótki wynik. Test integracji widzi końcowy błąd
w rzeczywistym żądaniu do skryptowanego dostawcy modelu.

### #72 — kod0 bez testów dawał fałszywe zaliczenie

`dev_test` uznawał kod0 i brak rozpoznanych porażek za sukces, nawet bez podsumowania albo
po wykonaniu0testów. Nowe regresje: pusty wynik, zero wybranych testów i same pominięte.
RED **6/3/0**. Obecnie `TestSummary.ok=true` wymaga rozpoznanego podsumowania, co najmniej
jednego zaliczonego testu, braku niezaliczonych i kodu0. Pozostałe przypadki są jawnie
niepotwierdzone. `ToolStatus::Ok` nadal oznacza wykonanie narzędzia, a nie zaliczenie testów.
Kod wyjścia znajduje się także w tekście, ponieważ model nie otrzymuje całego `data` wyniku.

Nieznane formaty runnerów pozostają niepotwierdzone; nie są automatycznie uznawane za błąd
kompilacji ani zaliczenie. Rozszerzanie parserów wymaga osobnych reprezentatywnych próbek.

### #70 — deterministyczny PID i chroniona łatka Core

Terminal na FakePty ma wymuszony PID4417, zgodny z cyframi testowego wejścia. Sprawdzamy
brak pełnej treści i ścisłą listę pól metadanych. **9/0/0**, Clippy0.
[Raport i konkretna łatka](2026-11-10-terminal-privacy.md).

Łatka `app-core/tests/computer.rs` nie została zastosowana w produkcie. Jest gotowa
do konkretnego przeglądu właściciela; w osobnym worktree **5/5** i Clippy0.
Nie poszerzono wcześniejszych zgód na Broker/SQLCipher.

## Rzeczywista kompilacja w runtime

`tests/coding_cycle.rs` wykonuje osobno dla Alfy, Bety, Gamy, Delty i własnej persony Nova:
rzeczywisty rustc → błąd E0308/kod1 → zapis poprawki → kompilację testu → test1/0 →
kompilację programu → uruchomienie z wynikiem42 → przegląd pliku i logów tylko do odczytu.
Każda persona ma rolę Koderki. Mutujące narzędzie jest niedostępne w turze weryfikacji.
Decyzje modelu są skryptowane; kompilator, pliki i procesy są rzeczywiste, w prywatnym tempdir.
**5/5 scenariuszy**. To nie dowodzi inteligencji modelu, działania Brokera produkcyjnego ani UAC.

## Prawdziwy Qwen8B — dwie małe próby na CPU

Osobny, ograniczony adapter HTTP i narzędzia jednego pliku; poza Core/Brokerem. Nie korzysta
z dowolnej powłoki lub danych użytkownika. Qwen3-8B Q4_K_M, llama.cpp b11559, CPU4wątki,
kontekst8192, thinking=false, temperature0/seed42, do224tokenów i6tur na przypadek.
Model sam wybiera argumenty i poprawkę. Każdy przypadek zakończył się po5turach/4wywołaniach.

| Próba | Fakty przed poprawką | Fakty po poprawce | HTTP: suma / najdłuższa tura / najpóźniejszy pierwszy fragment |
| --- | --- | --- | --- |
| Błąd typu | rustc kod1, E0308, brak uruchomionych testów | test1/0, program kod0 i `42` | 103,399s / 27,491s / 20,738s |
| Błąd logiki sumy | kompilacja0, testy1/2, program błędnie `45` | model zmienił `0..n` na `0..=n`; testy3/0, program0 i `55` | 126,423s / 39,943s / 21,008s |

Zachowano limity **per żądanie**: pierwszy fragment60s, przerwa30s, całość120s.
Nie zwiększano progów; czas126,423s jest sumą pięciu żądań, nie czasem jednego żądania.
Łącznie256wygenerowanych tokenów. Oba serwery po próbach zakończone kodem0.
SHA256 modelu: `d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785`.

To **2/2 małych sond**, nie pełny benchmark kodowania ani próba wszystkich person w Core.
Nie zamykają #41/#48 i wcześniejszej niezaliczonej próby Core CPU2/40.
Surowe odpowiedzi, żądania i wyjścia procesów: [ślady](artifacts/coding-model-traces.json).
Odtworzenie wymaga przypiętego modelu/serwera i nowego katalogu wyników; skrypty w archiwum scratch.

## Lokalne kontrole końcowe i commity

Wspólny przebieg po integracji12crate'ów (`agent-runtime`, `tools-dev`, `tools-shell`,
`ui-terminal`: contract/impl/fake), `--locked --all-features -j2`: **207/0/0**,62wyniki celów.
Wszystkie cele/funkcje Clippy tych samych crate'ów z `-D warnings`: **kod0**.
Wcześniejsze pełne kontrole modułów: runtime132/0/0, tools-dev14/0/0, terminal9/0/0;
audyt powłoki i tools-dev przed #72:62/0/0. Nie są to testy całego workspace.

| Commit kodu (drzewo identyczne z testowanym lokalnie) | Zakres |
| --- | --- |
| `ffb98c41b58838fa51150067faa2034c36b9b5d6` | ograniczona obserwacja i pierwsza próba kompilatora |
| `c081b93686db2fec036287bc31f469298832a027` | test metadanych terminala i łatka do przeglądu |
| `111bf1544e922561b21a79fcba4902be2b4a5deb` | cykl dla4wbudowanych person i własnej |
| `c6cf62622a728582981a8c835008ae9250089b54` | dowód wykonania testów, zamiast samego kodu0 |

Wspólny log testów SHA256: `354a22776a4c482db2062fece44330458f6a06e41c0c8ea1be622d35032fd3e7`.
Clippy: `c6ae48e1f41b767f42640efc36c2edd3325be30cf701478880dc591f6ab976bd`.
[Komendy, kody wyjścia, nieudane próby i hashe](artifacts/coding-workflow-validation.json).
Format, lint (Svelte/CSS/rozmiary/vendor), graf1809zależności i generowane103strony HTML:
**kod0**. Pierwszy odnośnik dokumentacji miał błędną kotwicę, poprawioną do wartości
generatora przed kontrolą końcową.

Pełny CI kolejnego heada należy sprawdzić w [PR24](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/24);
ten raport nie przypisuje mu wyników poprzednika. Bez merge lub wydania.

## Zachowane niepowodzenia i poprzednie CI

- Regresja obserwacji0/1, naprawiona. Nowy test fixture początkowo nie kompilował błędnego
  importu `Reversibility`; poprawiony, log zachowany. Pierwsze dwa błędy adaptera terminala
  opisano w jego raporcie.
- Próba RED #72 najpierw zatrzymana przez **No space left on device**, bez wykonania testów.
  To awaria środowiska, nie odtworzona regresja. Zwolniono nieaktywne, odtwarzalne binaria
  Cargo z zapisem SHA256; źródła, modele i logi zachowane. Kolejny RED faktycznie6/3.
- Baza `b4e2dbcd`: [CI180](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42792899943)
  **9/10**, Linux3567/0/26 i Windows3592/1/47, po1135wyników celów. Windowsowe `computer`
  4/1, błędny predykat4417. Pełnego zdarzenia CI nie zebrano, więc nie przypisujemy mu PID4417.
- Chromium w CI180 **315+315+315=945/945**. [Windows102](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42792900078)
  **4/4**, WebView2 **63/0/5**. To rzeczywista aplikacja Windows na runnerze, ale inny zakres
  niż sondy kodowania i sprzęt właściciela. Kompletnego przebiegu poprzednika nie anulowano.

## Uprawnienia, ograniczenia i dalszy odbiór

Rola Koderki już ma worktree/powłokę i narzędzia dev przez grupę shell. Pełny/Max dostęp
właściciel wybiera w istniejącym systemie; model nie podnosi go sam. Narzędzie wykonuje
polecenia, a prywatny terminal logowania jest osobnym kanałem użytkownika. Nie przejęto go
ani nie odczytywano tokenów. Instalacje winget/msiexec są analizowane przez istniejącą
powłokę; dokumentacja tools-system nadal oznacza specjalizowane operacje sterowników jako v2.

Nie potwierdzono na Cloud: sterowników/UAC/rebootu Windows, RTX2060 12GB, mikrofonu,
prawdziwego API/hybrydy ani zdalnej kontroli telefonu. Dostawca WWW niewybrany; podgląd
panelu telefonu nie jest działającym połączeniem. Bielik1.5B pozostaje pomocniczy; Qwen8B
główny,11B alternatywą tekstową. Nie odczytano kluczy i nie symulowano kont Luna/Haiku.

Dalej: konkretny przegląd łatki Core, niezależny przegląd przyrostów i własne pełne CI;
następnie prawdziwy runtime/narzędzia na Windows, GPU12GB oraz kontrolowana instalacja
pakietu/sterownika na maszynie testowej. Odbiór większych repozytoriów, wszystkich person
w prawdziwym modelu i hybrydy pozostaje otwarty. Nie deklarujemy ukończonej jakości Enterprise.
