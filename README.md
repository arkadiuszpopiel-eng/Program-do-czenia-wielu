# Alfa

Alfa to lekki, bardzo modułowy program dla Windows 11, który łączy modele lokalne, chmurowe API i zwykłe plany abonamentowe (przez oficjalne CLI, takie jak Claude Code i Codex) w jedną rozmowę — tekstową i głosową — z pełnym, kontrolowanym dostępem do komputera. Na start cztery agentki (Alfa, Beta, Gama, Delta) z własnymi głosami, pamięcią, samonaprawą i samodoskonaleniem; role przydzielane dowolnie. Program i pierwsza agentka noszą to samo imię.

## Stan projektu

**Stabilizacja MVP na sprzęcie** (stan na 2026-10-14, wersja 0.0.51). Kod wszystkich fal F0–F9 istnieje: 247 crate'ów Rust (ok. 400 tys. linii z testami) i interfejs Svelte 5 (ok. 75 tys. linii z testami); liczby testów z ostatniego pełnego biegu są w raporcie `target/verify/<wersja>/raport-wersja.md`. Z 108 kryteriów akceptacji 38 jest spełnionych w CI na atrapach, 57 czeka na pomiar na sprzęcie, korpusie albo kluczu, 11 nie ma zestawu lub implementacji, 2 czekają na decyzję właściciela — szczegóły w [`docs/STATUS.md`](docs/STATUS.md). Żadne kryterium nie jest jeszcze potwierdzone na prawdziwej maszynie; pierwszy test na laptopie (2026-10-07) znalazł błędy, których atrapy nie pokazały. Do zamknięcia MVP nie dochodzą nowe funkcje — kolejność prac: [`docs/KONTYNUACJA.md`](docs/KONTYNUACJA.md).

## Kluczowe założenia

| Założenie                   | Co z tego wynika                                                                                                                                                                                                                     |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Użytek osobisty**         | Brak dystrybucji, telemetrii i usług podpisywania; własny certyfikat lokalny; uproszczona zgodność                                                                                                                                   |
| **Budowa w 100 % przez AI** | Kod pisze Claude Code; recenzja w osobnej sesji ze świeżym kontekstem (ADR 0016). Człowiek uruchamia sesje, zatwierdza Jądro/Broker, ADR-y, makiety i wykonuje bramki ludzkie. Małe moduły, kontrakty, fake'i, sztywna Definition of Done |
| **Baseline sprzętowy**      | Ryzen 5 5600 · RX 7600 8 GB (AMD, bez CUDA) · 16 GB RAM · Windows 11 — tu program ma działać normalnie; wszystkie budżety i progi mierzone na baseline (emulowanym na desktopie RX 9070 XT; laptop RTX 4050 testuje ścieżkę CUDA)    |
| **Cztery agentki**          | Persony (imię, głos, charakter) są stałe; role (Dyrygentka, Mówczyni, Koderka, Krytyczka…) zmienne w obsadzie; głos idzie za personą, uprawnienia za rolą                                                                            |
| **Voice Suite**             | ~16 modułów `voice-*`: pełny dupleks, barge-in, korekty, cztery odrębne głosy młodych dorosłych, profil A działa lokalnie bez kluczy, lepsza jakość przez GPU/chmurę                                                                 |
| **Broker**                  | Osobna usługa na osobnym koncie Windows: tokeny zdolności, zatwierdzenia fizycznym wejściem, audyt append-only, kill-switch `Ctrl+Shift+F12`. Poziomy autonomii L0–L4 (start L3); twarde blokady działają także na L4                |
| **Działa bez kluczy**       | Lokalny LLM (llama.cpp) i lokalny głos jako domyślny „mózg" MVP; klucze i konta dodawane później bez restartu (`accounts-hub`)                                                                                                       |
| **Przenoszenie**            | Import/eksport do pliku `.alfa`, bez automatycznej synchronizacji                                                                                                                                                                    |

## Struktura repozytorium

```
apps/desktop/         powłoka Tauri 2 (src-tauri) + interfejs Svelte 5 (ui)
crates/               247 crate'ów: moduły jako trójki <moduł>-contract | -impl | -fake,
                      jądro core-*, kompozycja app-*, biblioteki lib-*  → docs/CATALOG.md
packages/ui-kit/      tokeny designu, komponenty, Storybook
packages/schemas/     JSON Schema zdarzeń i manifestu modułu
providers-catalog/    deklaratywny katalog dostawców (TOML)
evals/                zestawy akceptacyjne fal, spike'i, benchmarki głosu
scripts/              bramki jakości, instalacja i środowisko  → scripts/README.md
docs/                 plan, architektura, status, ADR, SPEC-i modułów  → docs/README.md
AGENTS.md  CLAUDE.md  README.md
```

Sidecary (`llama-server`, `whisper-server`, `piper`) i modele nie są w repozytorium — pobiera je menedżer
w aplikacji do `%LOCALAPPDATA%\Alfa`. Katalogów `sidecars/` i `plugins/` z planu §3.6 nie ma.

## Mapa dokumentów

Pełna mapa dokumentacji: [`docs/README.md`](docs/README.md). Najważniejsze wejścia:

| Pytanie | Plik |
| --- | --- |
| Co budujemy i dlaczego | [`docs/PLAN.md`](docs/PLAN.md) — zatwierdzony plan (wersja 5), źródło prawdy |
| Co jest zrobione, a co czeka | [`docs/STATUS.md`](docs/STATUS.md) — macierz 108 kryteriów akceptacji |
| Co robimy teraz i w jakiej kolejności | [`docs/KONTYNUACJA.md`](docs/KONTYNUACJA.md) |
| Który crate do czego służy | [`docs/CATALOG.md`](docs/CATALOG.md) — generowany katalog modułów |
| Jak pracują sesje AI | [`AGENTS.md`](AGENTS.md), [`docs/AI_WORKFLOW.md`](docs/AI_WORKFLOW.md) |
| Jak używać programu | [`docs/user-guide/`](docs/user-guide/README.md) |

## Roadmapa w skrócie

| Fala                                     | Jedno zdanie                                                                                                                                                  |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **F0 Fundament i spike'i**               | Repo, CI, kontrakty jądra, wzorzec modułu, `ui-kit` v0, ADR-y i time-boxowane spike'i (głos, most CLI, RAM Tauri, sprzęt, dane, Broker-UI).                   |
| **F1 Rdzeń czatu**                       | Czat z dostawcami API i lokalnym llama.cpp, sesje, hub kont i kluczy, ustawienia, zasobnik, Szybkie pytanie, eksport `.alfa` — bez narzędzi agentek.          |
| **F2 Głos rdzeniowy + agentki**          | Pełny potok głosowy z barge-in, cztery persony z głosami v0 bez kluczy, obsada ról, panele Głos i Agentki.                                                    |
| **F3 Safety Kernel + system**            | Broker, Broker-UI, watchdog, audyt, cofanie, kill-switch, poziomy L0–L4, pierwsze narzędzia fs/shell/schowek — **koniec MVP**.                                |
| **F4 Mosty i MCP** (∥ F5)                | Claude Code i Codex jako „opaque worker", karty zgodności, wbudowany terminal, klient i serwer MCP.                                                           |
| **F5 Agentki i głos rozszerzony** (∥ F4) | Wiele agentek równolegle, Scheduler, Marszałek, Kreator, wyzwalacze; słowa wywoławcze, weryfikacja mówcy, S2S, dyktowanie, czytanie na głos, pigułka głosowa. |
| **F6 Computer use**                      | UIA, wizja, wejście, przeglądarka, Office, helper UAC, panel Ekran; wymaga klucza API lub mostu jako „mózgu".                                                 |
| **F7 Pamięć pełna + transfer pełny**     | Cztery warstwy pamięci, konsolidacja, Inspektor, kaskadowe `forget`, kopie zapasowe.                                                                          |
| **F8 Samonaprawa i ulepszanie**          | Diagnosta, Ulepszacz z bramką ewaluacyjną i holdoutem, wtyczki Wasm, „Zdrowie systemu".                                                                       |
| **F9 Dopieszczenie**                     | Audyt a11y, wydajność i bateria, pentest, dokumentacja, „Co nowego".                                                                                          |

**MVP = F0–F3**: działa bez żadnych kluczy na baseline (lokalny model, lokalny głos, Broker, cofanie, kill-switch, `.alfa`).

## Jak zacząć

Wymagania na każdym komputerze: Windows 11, Visual Studio Build Tools z C++ i Windows SDK, WebView2, aktualny
sterownik karty graficznej. Pozostałe narzędzia (Rust, Node.js z pnpm, Strawberry Perl) mogą leżeć na dysku
przenośnym obok repozytorium w `..\alfa-narzedzia` — wtedy każde okno PowerShell zaczynasz od
`. .\scripts\portable-env.ps1`.

```powershell
. .\scripts\portable-env.ps1                                                    # narzędzia z dysku przenośnego
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\setup-dev.ps1     # co jest, czego brakuje
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\setup-dev.ps1 -Build
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\setup-dev.ps1 -Run
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\gates.ps1         # bramki jakości przed commitem
```

Pierwsze uruchomienie krok po kroku: [`docs/user-guide/11-pierwszy-test-na-pc.md`](docs/user-guide/11-pierwszy-test-na-pc.md).
Sesja AI zaczyna od `AGENTS.md`, `docs/STATUS.md` i `docs/KONTYNUACJA.md`.

## Licencja

Do ustalenia.
