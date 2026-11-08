# Jarvis i tryby — raport roboczy z Cloud, 8–9.11.2026

Plan: [54](../plan/54-jarvis-i-tryby-2026-11-08.md), karta [#22](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/22).
Gałąź: `feat/nightly-modes-jarvis`, podstawa PR [#20](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/20)
i poprawka Linux w PR [#23](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/23).
Środowisko: Linux Cloud x86_64, 4 CPU, Rust 1.94.1, pnpm 10.33.0, Chromium Playwrighta.
To raport autorski; wymagany przegląd przed scaleniem jest otwarty.

## Zmiany do przeglądu

- Ustawienie wspólne i cztery nadpisania profilu Standardowy / Jarvis; zapis i walidacja
  istniejącym magazynem konfiguracji. Panel Agentki udostępnia wybór każdej personie.
- Instrukcje w zwykłym czacie, transkrypcji głosu, przebiegu z narzędziami, delegacji Dyrygentki
  i zadaniu w tle. `Crew.persona_instructions` przenosi własny profil do delegacji runtime;
  starsze checkpointy domyślnie mają pustą mapę. Format werdyktu Krytyczki pozostaje bez tego profilu.
- Jawny zakres pięciu trybów: ta rozmowa / wszystkie rozmowy. Plan, wyjście z Planu i kłódka
  otwarta do końca rozmowy zachowują zakres sesji. Dotychczasowe zgody Brokera obowiązują.
- Licznik żądań odrzuca spóźniony wynik poprzedniej zmiany trybu. Dwa nowe testy były czerwone
  przed poprawką i zielone po niej.
- Poprawki układu: siedem zakładek panelu w jednym rzędzie i zawijanie długich przycisków
  ustawień przy szerokości 360 px. Nie zmieniano progów testów.
- Test mikrofonu `live_mic.rs` kompilowany wyłącznie na Windows: usuwa ostrzeżenia Linux,
  zachowując dotychczasowy test urządzenia na Windows.

## Wyniki potwierdzone w bieżącej sesji

| Sprawdzenie | Wynik | Zakres dowodu |
| --- | --- | --- |
| `app-core --test jarvis --test jarvis_voice` | 6 zaliczonych, 0 nieudanych | 4 persony, zapis i restart, narzędzia, delegacja, zadanie w tle, syntetyczne audio |
| `agent-runtime-contract`, `agent-runtime-impl`, `agent-runtime-fake` | 123 zaliczone, 0 nieudanych | wspólne kontrakty, zachowanie delegacji, zgodność starszych checkpointów |
| `cargo clippy --locked -p app-chat -p app-core --all-targets -- -D warnings` | zaliczone | zmienione moduły wraz z zależnościami |
| Jednostkowe UI (Vitest) | 568/568 | ponowiony przebieg po ostatnim porządku w UI |
| Jarvis E2E | 9/9 | 4 style × 2 motywy, osobny wybór każdej persony, axe, przejście do Głosu, zakres trybu |
| Dotychczasowe menu trybów E2E | 8/8 | menu i skróty |
| Układ: 360 px i zakładki | 3/3 | strony ustawień, pasek tytułu, jeden rząd zakładek |
| Svelte/TypeScript | 0 błędów, 0 ostrzeżeń | ponowiony przebieg po ostatnim porządku w UI |
| Lint UI | zaliczone | pełny lint przed ostatnim usunięciem kontrolki; końcowy przebieg po E2E |
| Format Rust, zależności, katalog funkcji, dokumentacja HTML | zaliczone | 1805 krawędzi, 611/611 funkcji z testem, 100 stron bez problemów |
| Pełne UI E2E | w toku, 856 przypadków | wynik dopisać po zakończeniu |
| `cargo test --locked --workspace` | w toku | wynik dopisać po zakończeniu |

Pełne logi robocze: `/workspace/scratch/`: `jarvis-core-voice.log`, `runtime-tests.log`,
`clippy-nightly.log`, `ui-vitest-final.log`, `ui-e2e-all.log`, `rust-workspace-tests.log`.
Obrazy `jarvis-{classic,glass,next,junior}.png` pokazują panel w Cloud.
Żaden z tych przebiegów nie jest odbiorem Windows/WebView2 ani odsłuchem prawdziwego mikrofonu.

## Rzeczywisty model: próby niezaliczone

Model: `Bielik-1.5B-v3.0-Instruct.Q8_0.gguf`, manifest `bielik-1.5b-v3.0-instruct-q8_0`.
SHA256: `e206ebbd9ee00cc8f14af5a096ad3f193a59608c63905316e8dc680c96655a60`.
Serwer: oficjalny przypięty llama.cpp **b11559**, kompilacja Linux, CPU (4 wątki).
Nie zmieniano deklaracji modelu `tools=false`. Próba jest headless, z rzeczywistym modelem,
ale z atrapami audio i portów Windows. Nie bada wykonywania narzędzi ani modelu-sędziego Z.8.

1. Bez jawnego limitu odpowiedzi: 7 ukończonych odpowiedzi, potem Delta/Standard/powtórzenie 2
   przekroczyła 240 s. Model mylił tożsamość: m.in. Alfa odpowiedziała „Jestem Jako”, Beta
   „Delta […] Darena”, inna Alfa „Jestem Krytyczka”. Próba **niezaliczona**.
2. Jawne 96 tokenów, temperatura 0: Alfa i Beta odpowiedziały; Gama/Standard/powtórzenie 1
   przekroczyła 120 s. Równocześnie działały kompilacje Rust i E2E, więc opóźnienia z tego przebiegu
   nie są baseline. Beta podała imię „Betta”. Próba **niezaliczona**.

Surowe częściowe odpowiedzi: [pierwsza próba](artifacts/jarvis-unbounded-partial.json)
i [próba pod obciążeniem](artifacts/jarvis-loaded-partial.json).
Brak błędu transportu nie oznacza poprawnej tożsamości ani wykonania zadania. Planowana próba
40 odpowiedzi (4 persony × 2 profile × 5 powtórzeń) wymaga ponowienia bez konkurujących obciążeń.
Test `jarvis_live.rs` jest jawnie pomijany w zwykłym CI i wymaga `ALFA_LIVE_*`.

## Niezależne problemy CI i dalsze kroki

CI PR #20: zbudowano powłokę Windows, testy UI jednostkowe i część integracji; cały przebieg
nie jest zielony. W logach znaleziono:

- Linux: `parse_snapshot` — wydzielona poprawka w PR #23. Kolejne ostrzeżenie w testach
  Brokera (`KillReason` / `KillSwitch` tylko na Windows) ma [gotową łatkę](patches/linux-broker-test-imports.patch).
  `AGENTS.md` zakazuje commitów do Brokera bez przeglądu człowieka; łatka nie została zastosowana
  do publikowanej gałęzi. Łatka ma format bez kontekstu (`git apply --unidiff-zero` po przeglądzie). Z tą lokalną kandydatką Clippy doszedł do następnego problemu w `live_mic.rs`;
  nie oznacza to zielonego pełnego workspace.
- Windows: dwa limity czasu `conductor_handoff`; polskie znaki w `exec_low` i `shell_utf8` zależne
  od strony kodowej OEM. Trzeba odtworzyć na Windows i ustalić poprawny kontrakt kodowania.
- UI: 842 zaliczone / 5 nieudanych — AltGr, dwa nakładania Junior, 360 px, dwa rzędy zakładek.
  Poprawiono dwa ostatnie przypadki; pełny przebieg nowej gałęzi trwa.
- Prawdziwa aplikacja Windows: 60 zaliczonych / 2 nieudane / 5 pominiętych — ukrywanie paneli
  oraz odczyt atrybutu wskaźnika terminala. Wymaga osobnej diagnozy.

Następna sesja: odczytać końce logów, zapisać wyniki, odtworzyć nowe błędy z regresją, potem
wybrać kolejny mały punkt planu 54. Nie uruchamiać LLM CPU równocześnie z testami czasowymi UI.
Reguły wymagają niezależnego przeglądu przed scaleniem; nie zamykano etapu J ani kryteriów sprzętowych,
więc wersja pozostaje 0.0.85. Harmonogram nocny ma cogodzinne wznowienia w odziedziczonym środowisku
Cloud; ostatnia iteracja o 07:00 UTC ma zebrać raport i zakończyć zmiany.
