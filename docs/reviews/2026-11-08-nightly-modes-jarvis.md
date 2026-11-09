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

## Ukończone ponowienie Rust i nowe UI

[Pełny Rust po #26–33](2026-11-08-rust-rerun.md), źródła988d13f3:
**3485 zaliczonych / 1 nieudany / 24 pominięte**. Workspace bez app-core3282/0/22;
app-core lib, wszystkie76 celów i doctesty203/1/2. Jedyna porażka dotyczy kolejki sesji w koszu
z testowym oknem Cofnij300ms ([#34](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/34));
[Test zegara](2026-11-08-queue-clock.md) po zmianie ma4/4,20 powtórzeń pary bez błędu
i4/4 sąsiednich testów. Nie wydłużono300ms ani nie uznano poprzedniego pełnego przebiegu za zielony. Chronione ostrzeżenie
Brokera nadal blokuje pełny Clippy i CI; istniejąca łatka pozostaje do przeglądu człowieka.

[Geometria menu i ustawień #32](2026-11-08-ui-geometry.md): **23/23 nowych E2E**, jednostkowe
**573/573** bez obciążenia, Svelte0/0, lint/bundle/IPC/dokumentacja zaliczone. Menu pięciu trybów
mieści się w niskim i wąskim oknie oraz po zmianie skali. Czerwone próby i zrzuty przed/po zachowane.
Pełny E2E nowego źródła do wykonania; stare856/856 i CI149 pozostają osobnymi wynikami.

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
Późniejszy pełny Rust988d13f3 opisano w sekcji powyżej.

[Kompozycja OCR/FFmpeg #33](2026-11-08-ocr-composition.md): 81 narzędzi +5 prób straży,
0 problemów na atrapach; wzorzec niezmieniony. Jarvis/głos6/6, diagnostyka i manifesty4/4,
app-agents32 zaliczone/3 pominięte. Ręczna zgoda w oknie Brokera pominięta, bez deklaracji odbioru.

Dalej: wykonać pełny E2E nowego UI, zaplanować końcowy pełny Rust po dalszych przyrostach,
wyjaśnić rozbieżności CI UI oraz problemy Windows. Nie uruchamiać LLM CPU równocześnie
z testami czasowymi UI. Wymagany niezależny przegląd pozostaje otwarty; wersja 0.0.85, etap J w toku.
Harmonogram ma cogodzinne wznowienia w Cloud; ostatnia iteracja o 07:00 UTC ma zebrać raport
bez rozpoczynania nowych zmian.

## Aktualizacja9.11: pełny UI i własna persona

[Raport #35–36](2026-11-09-custom-personas.md) zachowuje wyniki, początkową porażkę restartu
i jawne ograniczenie zakresu: odtworzenie definicji person/rol nie oznacza trwałej obsady rozmów (#37).
Luna z Kreatora dziedziczy Jarvisa i własne imię, także w delegacji. Po odtworzeniu katalogu i ponownym
przypisaniu roli działa po restarcie. Ukierunkowane testy: **15 zaliczonych / 0 nieudanych**;
pełny Rust po nowej zmianie produkcyjnej pozostaje do wykonania.

Pełny lokalny UI: **877 zaliczonych / 2 nieudane / 0 pominiętych**,37,8 minuty,
na `bd389207` (UI równoważne `7752ac70`). Obie porażki Planu Glass dotyczą oczekiwania5 sekund
na kartę końca odpowiedzi; nie podniesiono limitu. Osobno CI156: **879/879 E2E i573/573 jednostkowych**,
typy/lint/build/bundle/Storybook zielone. Windows Rust nadal trwał przy zbieraniu wyników.

WebView2 rehearsal77: **60 zaliczonych / 2 nieudane / 5 pominiętych**. Artefakt potwierdził brak
flagi instancji przy domyślnym porcie9222 i próbę schowania już zamkniętych Sesji w teście.
Poprawki harnessu są do wykonania. Nie ma dowodu awarii aplikacji w komunikacie zamknięcia strony
po timeoutcie i sprzątaniu testu. Wszystkie te wyniki są odrębne od odbioru sprzętu właściciela.

[Harness #38](2026-11-09-ui-test-harness.md): kontrolowany wariant wolnego strumienia odtworzył
brak karty Planu w5 sekundach. Zegar testowy doprowadza odpowiedź do końca; nowe8 wariantów wolnych
sprawdza brak karty przed końcem i obecność po nim. Po zmianie16/16 oraz10/10 powtórzeń Glass,
lint i check0/0. Nie zmieniono5 sekund ani progów wydajności. Pełny887 pozostaje do wykonania.
Przygotowano flagę instancji testowej także dla portu9222 i jawne otwarcie Sesji w scenariuszu
WebView2; te poprawki wymagają ponownego rzeczywistego Windows CI.

[Regresja testu zegara #39](2026-11-09-review-timer-windows.md): po anulowaniu CI156 log pokazał
zawieszenie nowego przypadku wygasania od00:30 do01:29. To własna regresja testu #34, mimo zielonych
prób Linux. Aktywne zadanie blokujące odtworzyło ją na Linux (watchdog30s, kod124); po ręcznym
przejściu299+1ms i wznowieniu zegara ten sam test przeszedł w0,32s, a zakresy mają11/11 i20 powtórzeń
bez błędu. Kod produkcyjny,300ms i progi pozostają dotychczasowe. Częściowe Windows:139 zakończonych
wyników celów,544/3/7, osobno3 indywidualne zaliczenia celu review; nie pełny wynik. Brak Brokera
w teście załączników otrzymał diagnostykę i zwolnienie starych uchwytów przed restartem; Windows do potwierdzenia.

[WebView2 #40](2026-11-09-webview-viewport.md): rehearsal78 na74ce7f99 ma61/1/5; plakietka
instancji naprawiona. Pozostały test dokowania działał w1028px, gdzie układ słusznie pokazuje
szufladę, bez przycisku dokowania. Tylko ten fragment dostaje teraz viewport1600×900 CDP,
z przywróceniem metryk i odłączeniem w finally. Helper3/3 na Chromium, check0/0, lint zaliczony.
Poprawka wymaga nowego Windows CI; emulowany viewport jest odrębny od natywnego okna i odbioru sprzętu.
Nowy pełny lokalny zestaw ma890 testów i pozostaje do wykonania.

## Ponowienie Bielika9.11, #41

[Raport i surowe wyniki](2026-11-09-jarvis-sentence.md): świeża baza236,32s, wariant instrukcji
211,03s, wariant końcowy238,42s. Każdy40/40 bez błędu transportu, obcięcia i pomyłki autora;
Jarvis20/20 imion, lecz0/20 jednego zdania w każdym przebiegu. Wszystkie40 tekstów identyczne.
Obie nieskuteczne zmiany app-chat cofnięto; zachowano łatki jako dowód, nie jako naprawę.
51/51 testów i Clippy przeszły osobno. Pełny Rust169a6004 działa na77 integracjach; wynik otwarty.

## Pełny Chromium CI159 i Windows80

Na169a6004 CI159 zaliczyło890/890 E2E (28,2min),573/573 jednostkowych/103 pliki, typy0/0,
lint/build/bundle/Storybook. Poprzedni lokalny877/2 zachowano oddzielnie. Windows80:60/1/6;
dokowanie #40 już zaliczone, nowy timeout kłódki Dziecko wymaga [diagnostyki #42](2026-11-09-webview-child-lock.md).
Nie zmieniono120s ani polityki. Linux Clippy nadal blokują chronione importy; pełny Linux Rust
jest uruchomiony osobno na77 integracjach. Windows Rust CI159 trwa; nie ma pełnego wyniku.

## Pełny Rust i własna Luna,03:15–03:23UTC

[Pełne wyniki](2026-11-09-full-validation.md): Linux169a6004 **3492/0/24**, wszystkie77 integracji,
workspace/lib/doc, komendy0. Windows CI159 **3516/5/45**;1121 oryginalnych wyników, wykluczono7
powtórzeń grep. Oba testy kolejki #39 przeszły; files nadal bez kernel_broker, mimo wiersza ready.
Dwa conductor_handoff i dwa kodowania Windows pozostają nieudane. Linux Clippy chroniony nadal czerwony.

[Nowa własna Luna #43](2026-11-09-live-custom.md):10/10 rzeczywistych tur Bielika w36,99s,
bez błędu/obcięcia/pomyłki autora, także po restarcie i ponownym przypisaniu roli. Standard5/5 jednego
zdania, Jarvis0/5 jednego zdania,5/5 poprawnych form imienia. Uprawnienia dotychczasowe, mikrofon
headless nieaktywny. Faktyczne instrukcje żądania routera zapisane w syntetycznym raporcie.

## Wspólny Jarvis w Kreatorze, #44

[Kontrolka i sprawdzenie](2026-11-09-builder-jarvis.md):9/9 E2E w26,4s,4 style/2 motywy iEN,
Beta dziedziczy, osobny Standard Alfy pozostaje. Typy0/0, lint/format/build/bundle zielone;
pełny899/899 naaf5a2bc5 zaliczony w38,5min,0 pominiętych i0 niestabilnych. Zakres wspólny opisany jawnie, mikrofon włączany osobno.
