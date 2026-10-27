# Przekazywanie wyników i dowodów między agentkami — 2026-10-24

Naprawa zgłoszenia, w którym Delta miała zapis 541/0 z kopii TEMP, a końcowy raport Alfy powtarzał brak testów, ponieważ weryfikatorka nie otrzymała wyników wykonawczyni.

## Zachowanie

- Kolejna agentka dostaje zwięzłe raporty poprzednich zleceń oraz hostowe wyciągi zapisów wykonania. Działa to dla osobnych tur modelu, zleceń w jednej partii i kontynuacji po restarcie.
- Zapis próby kompilacji/testów jest zbierany z faktycznych ToolUse + odpowiadających im ToolResult, nie z samego podsumowania modelu. Rozpoznawane są popularne komendy budowania/testowania; nie jest to pełny semantyczny parser wszystkich możliwych skryptów.
- Każdy zapis rozdziela wykonawczynię, domyślny katalog zadania oraz jawny argument cwd. Jawny cwd nie zostaje podmieniony na katalog projektu. Polecenie może dodatkowo wskazywać projekt w innej lokalizacji, np. kopię TEMP.
- Odmowa, anulowanie, wyczerpanie budżetu, wykryta pętla i niepotwierdzona weryfikacja nie są opisywane jako zwykłe ukończenie podzadania. Wcześniejsze wyniki pozostają zapisane.
- Raport końcowy ma oddzielny, generowany przez program wyciąg zapisanych komend i wyników. Model nie może usunąć tego wyciągu przez pominięcie go w swojej parafrazie. Sam kod 0 nie jest oznaczany jako potwierdzenie uruchomienia testów, a wynik TEMP nie potwierdza bieżącego oryginału.
- Kontekst jest ograniczony: do 8 zapisów w bieżącym stanie, do 3 wyciągów przy przekazaniu/wyświetleniu, z oznaczeniem skrócenia. Pełny dziennik pozostaje w lokalnych checkpointach. Nie dodano modelu podsumowującego ani dodatkowych tur API do obsługi tego mechanizmu.
- Powtórzone identyfikatory wywołań dostawcy między turami są rozdzielane numerem wystąpienia; wynik nie zostaje przypisany do wcześniejszego wywołania o tym samym id. Scalanie przy ponownym odczycie jest idempotentne.
- Sekrety są redagowane przed przekazaniem/wyświetleniem wyciągu. Raporty agentek i stdout pozostają niezaufanymi danymi, nie podnoszą uprawnień recenzentki.

## Pochodzenie poleceń

Przegląd wykrył dodatkową lukę: custom-child Dyrygentki ma depth=1 bez natywnego parent id. Runtime rozpoznaje teraz delegację także przez depth/owner_call i nie dodaje celu napisanego przez model do zaufanego polecenia właściciela. Autentyczny cel właściciela jest przekazywany osobno. Wznowienie starszego custom-child usuwa dawne awansowanie celu modelu do owner trust, zachowuje pierwotny katalog oraz taint i niezaufany kontekst. Nie zmieniano poziomów L4, polityki Brokera ani profili uprawnień użytkownika.

## Blokada narzędzi po restarcie

Nowy test pełnego restartu wykrył niezależną przeszkodę: po operacji powłoki dziennik cofania nie dawał się otworzyć (`invalid type: string "0", expected u32`), przez co narzędzia były niepodłączone. Przyczyną była deserializacja liczbowych kluczy Commit.posts przez wewnętrznie tagowany enum.

Poprawiono wyłącznie deserializację: istniejące kanoniczne klucze tekstowe JSON są odczytywane jako u32, pozostaje obsługa liczbowych indeksów, a błędne, przepełnione i zduplikowane indeksy są odrzucane. Zapis JSON, ścieżki, hashe i kontrola konfliktów przy cofaniu pozostają bez zmian. Nie usuwano i nie przepisywano dziennika.

Odczyt istniejącego dziennika właściciela z nowym dekoderem: 110 rekordów, bez niepełnego ogona; tryb tylko do odczytu (`target/handoff-existing-journal-check.log`). To sprawdzenie formatu, nie destrukcyjny test cofania na danych właściciela.

## Potwierdzenie

- RED: brak raportu u recenzentki (`target/handoff-red.log`), odmowa błędnie traktowana jako sukces (`target/handoff-outcome-red.log`).
- RED dodatkowych granic: powtórzone id, jawny cwd, proweniencja celu, trwały commit dziennika (`target/handoff-reused-id-red.log`, `handoff-cwd-red.log`, `handoff-provenance-red.log`, `handoff-journal-red.log`).
- Końcowe testy app-chat, agent-runtime-impl, undo-journal-contract i undo-journal-impl: 148/148; `target/handoff-packages-final.log`.
- Końcowa regresja rdzenia (handoff, kontynuacja, profile, pipeline, kontekst projektu): 25/25; `target/handoff-regressions-final.log`.
- Dedykowane handoff: 4/4, ponownie po finalnej korekcie tekstu; `target/handoff-final-tests.log`.
- Scenariusze: zwykłe przekazanie raportu, dwie delegacje w tej samej turze, wynik komendy przed późniejszą odmową, limit i pełne zniszczenie/odtworzenie runtime z rzeczywistym wywołaniem recenzentki po wznowieniu. Liczba żądań potwierdza brak powtórnego uruchomienia ukończonej Delty.
- Test Windows wykonuje bezpieczny Write-Output z oznaczonym wynikiem demonstracyjnym, po którym skryptowany dostawca zwraca odmowę. To test przenoszenia wyjścia narzędzia, nie ponowne uruchomienie prawdziwych 541 testów Tłumacza AI.
- Test trwałości cofania: snapshot → zmiana → commit → reopen → undo odtwarza pierwotną treść. Niekanoniczne, przepełnione i powtórzone indeksy są odrzucane, format zapisu zgodny z dotychczasowym.
- Clippy all-targets dla zmienionych pakietów i kontrola narzędzia diagnostycznego: bez ostrzeżeń; `target/handoff-clippy-final.log`, `target/handoff-inspector-clippy.log`.
- Bramka rozmiarów: 0 naruszeń; nowe pliki również sprawdzone poniżej 400 linii. app-chat pozostaje poniżej 8000 linii.
- Niezależny przegląd statyczny zamknął ustalenia dotyczące cwd, proweniencji celu i zgodności dziennika.
- Windows build --locked --offline zakończony sukcesem; `target/handoff-native-final.log`.

Nie wykonywano płatnych API, nowego testu natywnego GUI właściciela ani kompilacji/testów oryginalnego katalogu Tłumacza AI. Jego dotychczasowy wynik 541/0 pozostaje wynikiem wcześniejszej kopii TEMP. Nie edytowano projektu Tłumacza AI. Stare błędne odpowiedzi Alfy pozostają w append-only historii; poprawka działa dla nowo wykonywanych/wznawianych zadań i zapisów. Nie uruchamiano pełnego cargo test --workspace.

## Wdrożenie

Podmieniono EXE i DLL w `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\Alfa-uruchomiona` o 2026-10-24T16:00:38.2823979+02:00. Sumy SHA-256 zgodne z buildem. Kopia sprzed tej naprawy: `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\alfa\target\handoff-rollback-20261024-154501`. Program pozostawiono zamknięty.

- `alfa-desktop.exe`: `7EB68A6B1BDBB2F3F80D55595CBBE9226989B1D46594F56F8157258661431D10`
- `alfa_desktop_lib.dll`: `9375E0093D44EDD61771F177DE05A820A2FA23DCFB3D899B3F3D56DFBF56B49C`
