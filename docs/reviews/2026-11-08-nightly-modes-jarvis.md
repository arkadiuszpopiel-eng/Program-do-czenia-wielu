# Jarvis i tryby — raport roboczy z Cloud, 8–9.11.2026

Plan: [54](../plan/54-jarvis-i-tryby-2026-11-08.md), karta [#22](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/22).
Gałąź: `feat/nightly-modes-jarvis`, podstawa PR [#20](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/20)
i poprawka Linux w PR [#23](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/pull/23).
Środowisko: Linux Cloud x86_64, 4 CPU, Rust 1.94.1, pnpm 10.33.0, Chromium Playwrighta.
To raport autorski; wymagany przegląd przed scaleniem jest otwarty.

Aktualizacja 21:03 UTC: stare procesy zakończyły się bez końcowego wyniku pełnego UI i Rust.
Wznowienie oraz następna regresja: [Plan bez rozmowy](2026-11-08-plan-without-session.md).

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
| Pełne UI E2E | **856/856**, 48,6 min | kompletna wznowiona próba na `9b8b7416` |
| Pełny zakres Rust w partiach | **3461 zaliczonych / 20 nieudanych / 24 pominięte** | `9b8b7416`; [wyniki i dalsze poprawki](2026-11-08-catalog-platform.md) |

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
Brak błędu transportu nie oznacza poprawnej tożsamości ani wykonania zadania. Próba
40 odpowiedzi (4 persony × 2 profile × 5 powtórzeń) została później ukończona bez konkurujących obciążeń; szczegóły poniżej.
Test `jarvis_live.rs` jest jawnie pomijany w zwykłym CI i wymaga `ALFA_LIVE_*`.

Po złożeniu poprawki katalogu sześć testów Jarvisa i syntetycznego głosu ponownie przeszło.
Ręczna próba modelu zapisuje teraz także autora tury, `truncated` i zużycie tokenów. Autor tury
jest sprawdzany względem adresatki. Pole `mentions_own_name` oznacza jedynie literalne wystąpienie
imienia w tekście; nie stanowi semantycznej oceny tożsamości ani dowodu wykonania zadania.

## Ukończona próba bez obciążenia i następny przyrost

Na `e8baf1da` Bielik ukończył **40/40 tur w 215,88 s**, bez błędu transportu i obcięcia,
autor zgodny w 40/40. Pozostały błędne formy „Betta” / „Gamma” i nadmiar zdań.
[Raport form imienia](2026-11-08-jarvis-identity.md) zachowuje surowe odpowiedzi i opisuje poprawkę #31.
Po poprawce #31 ta sama próba40/40: imię Jarvisa poprawne20/20, ale jedno zdanie0/20;
pełna jakość nadal niezaliczona. Nie utożsamia powodzenia integracji z zaliczeniem jakości. UI jednostkowe po poprawce #25
powtórzone bez obciążenia: **573/573**; wcześniejsze 572 zaliczone / 1 przekroczony budżet
wyszukiwania pod obciążeniem pozostaje zapisanym niepowodzeniem. Lint i dokumentacja zaliczone.

## Niezależne problemy CI i dalsze kroki

Ukończone CI [#149](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42524101617)
i [rehearsal #70](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42524101627)
na `25c4e788`: [zestaw wyników](artifacts/ci-149-results.json). Całość nie jest zielona.

- Windows Rust: **3504 zaliczone / 5 nieudanych / 45 pominiętych**. Dwa limity `conductor_handoff`,
  `files::attachment_taint_reaches_broker_and_survives_restart` (brak Brokera w procesie),
  polskie znaki w `exec_low` i `shell_utf8` zależne od strony kodowej OEM.
- Linux: Clippy zatrzymany przez importy `KillReason` / `KillSwitch` w teście Brokera; testy pominięte.
  [Gotowa łatka](patches/linux-broker-test-imports.patch) wymaga przeglądu człowieka zgodnie z
  `AGENTS.md` i nie jest zastosowana do publikowanej gałęzi (`git apply --unidiff-zero` po przeglądzie).
- UI Chromium CI: **852 zaliczone / 4 nieudane**. Radio zakresu rozmowy poza viewportem,
  nakładanie opisów wtyczki w obu motywach Junior i pasek akcji API poza oknem 360 px.
  Lokalny pełny przebieg **856/856** na `9b8b7416` jest osobnym wynikiem; różnice wymagają diagnozy.
- Windows/WebView2 na runnerze GitHub: **60 zaliczonych / 2 nieudane / 5 pominiętych**.
  Ukrywanie paneli zamyka stronę; brak atrybutu instancji testowej. To nie odbiór na sprzęcie właściciela.
- Zielone: powłoka Tauri (build, Clippy, testy produkcyjne i e2e), cargo-deny, UI lint/typy/jednostkowe/
  bundle/Storybook, silniki CPU na żywo, katalog URL/hash i instalatory PowerShell 5.1/7.

[Poprawki danych testowych #27–30](2026-11-08-linux-fixtures.md): **95 zaliczonych / 0 nieudanych / 3 pominięte**
w ukierunkowanych zakresach, Clippy zielone. Pierwszy przebieg55/1 zachowano i wyjaśniono.
Pełny Rust po zmianach nie był jeszcze ponowiony.

Dalej: rozwiązać `tool_diag` (#33), ponowić pełny Rust,
wyjaśnić rozbieżności CI UI oraz problemy Windows. Nie uruchamiać LLM CPU równocześnie
z testami czasowymi UI. Wymagany niezależny przegląd pozostaje otwarty; wersja 0.0.85, etap J w toku.
Harmonogram ma cogodzinne wznowienia w Cloud; ostatnia iteracja o 07:00 UTC ma zebrać raport
bez rozpoczynania nowych zmian.
