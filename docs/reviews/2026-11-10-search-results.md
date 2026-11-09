# Wyniki wyszukiwania — sekrety i granice danych, 10.11.2026

Karta[#56](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/56),
baza`23300f82fa6a9c940ab3121a4dfa9d908f28d4c9`, osobny worktree`/workspace/wt-search-results`.
Zlecenie właściciela: rozwój do10.11.05:30 Europe/Warsaw, potem końcowe testy i raport,
STOP06:00. Kontynuacja i końcowe rozliczenie zapisane jako ograniczone czasowo zadania.

## Wykryte problemy

`Core::search` redagował tytuł i opis wyniku, ale przekazywał URL wprost do tekstu oraz JSON.
Wynik dostawcy z `?token=tajny123` trafiał więc do odpowiedzi narzędzia. To syntetyczny sekret
testowy; nie odczytywano kluczy właściciela. Ponadto narzędzie ufało argumentowi `max` po stronie
dostawcy i nie ograniczało liczby otrzymanych trafień. Trzeci problem: ograniczony tekst wyniku
nie ograniczał pól JSON — próbka dla budżetu512 miała23029 znaków w polach strukturalnych.
Wyniki zawierające localhost,HTTP,dane logowania lub znaki sterujące nie były filtrowane.

## Naprawa

Projekcja źródeł w `tools-net-impl/src/search_results.rs` egzekwuje liczbę wyników niezależnie
od dostawcy. Używa istniejącego `checked_target` do walidacji źródła HTTPS/publicznego i
rozpoznawania sekretów; odrzuca także URL ze znakami sterującymi. Niedozwolonego lub za długiego
URL nie skraca ani nie zmienia w fikcyjny adres. Bezpieczny URL pozostaje bajt w bajt.

Istniejący `output_max_chars` ogranicza także sumę znaków `url/title/snippet` w wyniku JSON.
Tytuł i opis są redagowane, a następnie skracane na granicy znaku Unicode. W tekście narzędzia
jest informacja, gdy część źródeł pominięto lub skrócono. Prawdziwa pusta lista jest odróżniona
od listy, z której odrzucono wszystkie źródła. Zachowano taintWeb, zgodę Brokera i zwalnianie
tokenu po wyszukiwaniu; do walidacji źródeł nie wykonuje się żądań sieciowych.

Nie zmieniono globalnych polityk, deny-list, progów evals, schematów lub zależności. To użycie
istniejącej walidacji na granicy wyniku narzędzia. Nie jest to jeszcze produkcyjna wyszukiwarka.
SearchPort zwraca już zaalokowany wektor: ta poprawka ogranicza projekcję, a nie wejściową pamięć
lub dekoder przyszłego adaptera. Kontrola URL bez żądania nie sprawdza aktualnego DNS ani treści
źródła; rzeczywisty odczyt nadal wymaga net_fetch i jego kontroli Brokera/DNS.

## Wyniki i ograniczenia

| Próba | Wynik |
| --- | --- |
| RED przed poprawką | 0 zaliczonych/4 nieudane,kod101:sekret z URL,nadmiar wyników,niedozwolone źródła,nadmierny JSON |
| Pierwszy GREEN | 4/4,kod0 |
| Końcowe przypadki search_results | 6/6,w tym zero/małe budżety i rozróżnienie pustych/odrzuconych wyników |
| Pełne tools-net-contract/impl/fake,all-features | 35 zaliczonych/0 nieudanych/0 ignorowanych,10 wyników celów z pustymi doctestami |
| Pierwszy Clippy | kod101,dwa field_reassign_with_default wyłącznie w nowych danych testowych; log zachowany |
| Końcowy Clippy all-targets/all-features | kod0,bez ostrzeżeń,1,74s; inicjalizatory poprawione,bez wyciszania lintów |
| Format i lint | kod0,0 naruszeń;29 plików vendora zgodnych |
| Generatory dokumentacji | 102 strony,--check i--verify0 problemów |

Przypadki obejmują wadliwy dostawca ignorujący max_results, URL z wykrytym tokenem/hasłem,
publiczny poprawny URL po odrzuconych trafieniach,localhost/prywatny IP,HTTP,javascript,dane
logowania,znaki sterujące,duże tytuły/opisy i znaki wielobajtowe. Sieć to atrapa, a Broker to
harness istniejącego silnika. Nie jest to żywe API,dowód braku każdego możliwego sekretu,
ranking źródeł,odbiór GPU12GB lub mikrofonu.

```sh
export CARGO_TARGET_DIR=/workspace/target-nightly
export CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo +1.94.1 test --locked -p tools-net-contract -p tools-net-impl -p tools-net-fake --all-features -j2
cargo +1.94.1 clippy --locked -p tools-net-contract -p tools-net-impl -p tools-net-fake --all-targets --all-features -j2 -- -D warnings
cargo +1.94 fmt --all -- --check
```

Logi lokalne:`/workspace/scratch/search-results/`.
[Zapis walidacji i odciski](artifacts/search-results-validation.json).

## Potwierdzona baza i dalszy odbiór

[CI170](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42693934313)
na23300f82 ma10/10 zadań success:Linux3514/0/26,Windows3540/0/47,po1128 wyników celów,
Clippy0,Vitest573/573/103pliki,Svelte0/0 i pełne905 E2E w częściach302+302+301
(9,7/8,5/8,1min). Końcowa bramka UI zaliczona dopiero po wszystkich częściach.
[Windows92](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42693934101)
ma4/4success,WebView2 63/0/5 w3,0min. To zamyka potwierdzenie poprzednich poprawek Routera/CI,
nie jest jeszcze pełnym CI nowego kodu wyników wyszukiwania.

Po publikacji wymagane nowe CI i niezależny przegląd. Nadal otwarte #41/#48, rzeczywisty
dostawca WWW, badanie jakości i cytowań oraz sprzęt/API/mikrofon właściciela. Nie scalono PR.
