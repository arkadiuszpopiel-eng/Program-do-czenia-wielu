# Kolejki, koszty i trwała kontynuacja zadań — 2026-10-24

Wdrożono lokalnie. EXE i DLL podmienione w `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\Alfa-uruchomiona` o 2026-10-24T12:26:24.4001451+02:00; SHA-256 plików docelowych zgodne z buildem. Poprzednie pliki: `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program\alfa\target\continuity-rollback-20261024-122332`. Dane sesji i ustawienia właściciela nie były usuwane. Program pozostawiony zamknięty.

## Zachowanie

- „Wszystkie konta” ma pierwszeństwo przed domyślnym przypisaniem agentki. Kolejka jest ścisła i obejmuje oddzielne aktywne konta tego samego dostawcy. Po wykryciu braku postępu Router pomija nieskuteczny prefiks kolejki, zamiast wracać na początek. Zmiana modelu nie obchodzi odmowy właściciela ani zasad narzędzia.
- Limit kroków to pula 5–500 kroków. Po wyczerpaniu kroków/czasu: lokalny zapis i pytanie o kolejną pulę. Trzy osobne przełączniki w Ustawienia → Agentki dotyczą kroków, aktywnego czasu i oczekiwania na zgodę; domyślnie wyłączone. Można przekraczać kolejne pule bez sztywnego końcowego limitu godzinnego, jeśli właściciel zezwolił. Limity kosztu nadal obowiązują.
- Czekanie na zatwierdzenie nie zużywa czasu aktywnej pracy. Automatyczne przedłużanie czekania zachowuje obowiązek rzeczywistego zatwierdzenia. Odmowa/anulowanie zatrzymują pracę.
- Checkpointy są atomowo zapisywane w szyfrowanej bazie sesji. Zachowują historię, wyniki, rozpoczęte i zakończone operacje, zużycie, stan Dyrygentki i kursor pipeline. Operacja przerwana między efektem a zapisem ma jawnie nieznany stan do sprawdzenia; nie deklarujemy dokładnie jednokrotnego wykonania zewnętrznych efektów w tej nierozstrzygalnej granicy awarii.
- Wznowienie odtwarza dotychczasowe efekty i korzysta z bieżących profili dostępu. Stary przycisk kontynuacji nie uruchamia ponownie ukończonej kontynuacji. Anulowane zadanie nie staje się wstrzymanym. Nieczytelny zapis powoduje błąd odzyskiwania, zamiast utworzenia nowego planu bez historii.
- Po awarii przed zapisem tury odpowiedzi zapisane wykonanie można wznowić z Replay. Jawne ponowienie zadania w tle po limicie zachowuje historię poprzedniej próby.
- Rozliczenie kontynuacji jest przyrostowe. Zapis uwzględnia rzeczywiste modele/konta; techniczny alias konta nie omija limitów kosztowych i compliance dostawcy.
- Starsze duże wyniki narzędzi są skracane tylko w widoku wysyłanym modelowi. Instrukcje właściciela oraz lokalny dziennik zostają. `task_recall` udostępnia żądany zakres starszego wyniku jako niezaufane dane. To nie wymaga dodatkowego wywołania modelu podsumowującego.
- Identyczny zakres, który już przekroczył limit snapshotu, nie jest ponownie skanowany. Powtórzenie uruchamia ochronę przed brakiem postępu; model otrzymuje wskazówkę zawężenia cwd albo użycia operacji na konkretnym pliku.

## Potwierdzenie

| Kontrola | Wynik / dowód |
| --- | --- |
| Rust: runtime, router, kontrakt routingu, gate narzędzi, store i shell | 180 testów, 0 porażek; `target/continuity-regressions.log` |
| Końcowe testy kontynuacji runtime | 5/5; zapis przed mutacją, częściowy stream, automatyczny czas, wznowienie zapisu i ochrona przed kosztowną pętlą snapshotu; `target/continuity-final-runtime.log` |
| Adapter zadań w tle | 9/9; `target/continuity-executor-final.log` |
| Kolejki kont i orkiestracja/pipeline | 16/16; `target/continuity-final-core.log` |
| Ostatnia regresja uprawnień, routingu etapów i kontynuacji | 13/13; `target/continuity-final-access.log` |
| Nagła awaria | Zniszczenie runtime podczas odpowiedzi, zanim powstała tura agentki; ponowne otwarcie tego samego profilu i odtworzenie wyników bez powtórzenia zapisu pliku |
| Kolejność wielu kont | Lokalny serwer HTTP, również drugie konto tego samego dostawcy; nie są to płatne API |
| Vitest | 491/491; `target/continuity-vitest.log` |
| Playwright / Edge | 7/7, atrapa backendu; menu i centrowanie agentek, ustawienia z prawej, 500 kroków, pipeline, dialog kontynuacji i trzy zgody; `target/continuity-ui-regression.log` |
| Svelte/TypeScript | 0 błędów i ostrzeżeń; `target/continuity-ui-final.log` |
| Kontrakt IPC | 3/3; `target/continuity-schema.log` |
| Clippy | Sprawdzone biblioteki zmian z `-D warnings`, bez ostrzeżeń; `target/continuity-clippy.log` |
| Rozmiary i format | Bramka rozmiarów czysta; dodatkowo uwzględniono nowe pliki spoza indeksu Git. Rustfmt na zmienionych źródłach |
| Windows | Build `--locked --offline`, Tauri custom-protocol/no-console, profil dev, CARGO_PROFILE_DEV_DEBUG=0; `target/continuity-native-final.log` |
| Przegląd | Niezależny przegląd statyczny; zgłoszone problemy zapisu, kosztów, profili, podwójnego wznowienia i anulowania poprawione; końcowe dwa ustalenia potwierdzone jako zamknięte |

## Granice potwierdzenia

Nie wykonano płatnych wywołań API ani godzinnego zadania na prawdziwych modelach. Zmniejszenie rozmiaru kontekstu sprawdzono syntetycznie — nie jest to pomiar procentu oszczędności rachunku. Czas i timeouty są sprawdzane deterministycznie, także z zegarem wirtualnym. Test nagłego restartu niszczy runtime procesu testowego; nie jest testem odcięcia zasilania ani fizycznej awarii dysku.

Native build jest skompilowany i podmieniony, lecz w tym etapie nie uruchamiano GUI właściciela. Testy interfejsu korzystały z Edge i atrap backendu. Nie uruchamiano pełnego `cargo test --workspace` ani testów sprzętowych. Pełny dysk/błąd zapisu ma oddzielny test fail-closed; nie wykonywano destrukcyjnego zapełniania dysku właściciela.

## Pliki i cofnięcie

Checkpointy trafiają do zaszyfrowanych baz sesji w katalogu profilu aplikacji, standardowo `%LOCALAPPDATA%\Alfa\sessions` (profil przenośny/testowy korzysta ze swojego katalogu). Mechanizm checkpointów używa lokalnej bazy; nie dodano nowej usługi chmurowej.

Przy zamkniętym programie można przywrócić tylko EXE i DLL z powyższego katalogu rollback do `Alfa-uruchomiona`. Nie usuwać baz sesji ani ustawień. Starszy program nie udostępnia nowego wznowienia; zapisane checkpointy pozostają lokalnie.

Sumy wdrożonych plików:

- `alfa-desktop.exe`: `4767C46CF71D675A2DF0EF96189E4DD9FC23F6482A336E1091DD78EB10FC5235`
- `alfa_desktop_lib.dll`: `4F7E9D74F8C1DC7208CD5C54E21D3D031D77D75A7E8D2468CDE0107C4C8622BA`

## Późniejsza regresja delegowania

Zrzut właściciela ujawnił brak delegate_task przy modelu spoza statycznej mapy capabilities. Przyczynę, nowe testy HTTP i poprawione wdrożenie opisuje [raport regresji](../conductor-capabilities-2026-10-24/REPORT.md). To wdrożenie zastępuje podane wyżej sumy plików.
