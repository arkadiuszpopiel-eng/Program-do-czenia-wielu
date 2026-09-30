# Alfa

Alfa to lekki, bardzo modułowy program dla Windows 11, który łączy modele lokalne, chmurowe API i zwykłe plany abonamentowe (przez oficjalne CLI, takie jak Claude Code i Codex) w jedną rozmowę — tekstową i głosową — z pełnym, kontrolowanym dostępem do komputera. Na start cztery agentki (Alfa, Beta, Gama, Delta) z własnymi głosami, pamięcią, samonaprawą i samodoskonaleniem; role przydzielane dowolnie. Program i pierwsza agentka noszą to samo imię.

## Stan projektu

**Faza planowania.** Repozytorium zawiera wyłącznie dokumentację: zatwierdzony plan programu i dokumenty pochodne (workflow AI, kryteria akceptacji, model zagrożeń, reguły dla agentów). Nie ma jeszcze kodu. Pierwszy kod powstanie w fali F0 (§ „Jak zacząć").

## Kluczowe założenia

| Założenie | Co z tego wynika |
|---|---|
| **Użytek osobisty** | Brak dystrybucji, telemetrii i usług podpisywania; własny certyfikat lokalny; uproszczona zgodność |
| **Budowa w 100 % przez AI** | Kod piszą Claude Code (Opus 5.5) i Codex CLI (GPT-6 Sol); człowiek uruchamia sesje, zatwierdza Jądro/Broker, ADR-y, makiety i wykonuje bramki ludzkie. Małe moduły, kontrakty, fake'i, przegląd krzyżowy, sztywna Definition of Done |
| **Baseline sprzętowy** | Ryzen 5 5600 · RX 7600 8 GB (AMD, bez CUDA) · 16 GB RAM · Windows 11 — tu program ma działać normalnie; wszystkie budżety i progi mierzone na baseline (emulowanym na desktopie RX 9070 XT; laptop RTX 4050 testuje ścieżkę CUDA) |
| **Cztery agentki** | Persony (imię, głos, charakter) są stałe; role (Dyrygentka, Mówczyni, Koderka, Krytyczka…) zmienne w obsadzie; głos idzie za personą, uprawnienia za rolą |
| **Voice Suite** | ~16 modułów `voice-*`: pełny dupleks, barge-in, korekty, cztery odrębne głosy młodych dorosłych, profil A działa lokalnie bez kluczy, lepsza jakość przez GPU/chmurę |
| **Broker** | Osobna usługa na osobnym koncie Windows: tokeny zdolności, zatwierdzenia fizycznym wejściem, audyt append-only, kill-switch `Ctrl+Shift+F12`. Poziomy autonomii L0–L4 (start L3); twarde blokady działają także na L4 |
| **Działa bez kluczy** | Lokalny LLM (llama.cpp) i lokalny głos jako domyślny „mózg" MVP; klucze i konta dodawane później bez restartu (`accounts-hub`) |
| **Przenoszenie** | Import/eksport do pliku `.alfa`, bez automatycznej synchronizacji |

## Struktura repozytorium

Docelowa (z planu §3.6):

```
apps/desktop/         Tauri 2 shell + ui/ (Svelte 5)
crates/<moduł>-contract|-impl|-fake
crates/core-*         jądro
sidecars/             ciężkie moduły jako osobne procesy (whisper.cpp, TTS…)
plugins/              źródła wtyczek Wasm + WIT
packages/ui-kit/      tokeny designu, komponenty, Storybook
packages/schemas/     JSON Schema zdarzeń i konfiguracji
providers-catalog/    deklaratywny katalog dostawców
evals/                zamrożone zestawy akceptacyjne fal (hash), benchmarki głosu
docs/                 PLAN.md, ARCHITECTURE.md, AI_WORKFLOW.md, ACCEPTANCE.md, VOICE.md, PERSONAS.md, UI.md,
                      THREAT_MODEL.md, ADR/, modules/<m>/SPEC.md, formats/, vendor/, compliance/
AGENTS.md  CLAUDE.md  README.md
```

Teraz w repo:

```
AGENTS.md  CLAUDE.md  README.md
docs/PLAN.md  docs/AI_WORKFLOW.md  docs/ACCEPTANCE.md  docs/THREAT_MODEL.md
```

## Mapa dokumentów

| Plik | Co zawiera |
|---|---|
| `docs/PLAN.md` | Zatwierdzony plan programu (wersja 5): decyzje, architektura, dostawcy, Voice Suite, computer use, bezpieczeństwo, agentki, pamięć, UI, roadmapa F0–F9, ryzyka — źródło prawdy |
| `docs/AI_WORKFLOW.md` | Jak projekt jest budowany przez AI: wykonawcy, przegląd krzyżowy, środowisko i runnery, kolejność pracy nad modułem, bramki CI, Definition of Done, fake'i, pierwsze dwa tygodnie, bramki ludzkie, szablony karty zadania i PR |
| `docs/ACCEPTANCE.md` | Zestawy i progi akceptacyjne fal F0–F9 (identyfikatory kryteriów, progi, maszyny, kto weryfikuje), zasada zamrażania w `evals/`, definicja MVP i scenariusz MVP bez kluczy |
| `docs/THREAT_MODEL.md` | Aktywa, aktorzy, granice zaufania, „lethal trifecta", 28 scenariuszy ataku z kontrolami i testami, twarde blokady Jądra, poziomy autonomii, zgodność tras abonamentowych, próg red-team |
| `AGENTS.md` | Reguły dla agentów AI (≤ 100 linii): stos i wersje, format modułu, reguły twarde, kolejność pracy, DoD w skrócie, limity, komendy, gdzie szukać |
| `CLAUDE.md` | Cienki plik z `@AGENTS.md` (Windows bez symlinków) |
| *(planowane)* `docs/ARCHITECTURE.md`, `docs/VOICE.md`, `docs/PERSONAS.md`, `docs/UI.md`, `docs/ADR/`, `docs/modules/*/SPEC.md`, `docs/vendor/`, `docs/compliance/` | Powstają w F0 według §20 planu |

## Roadmapa w skrócie

| Fala | Jedno zdanie |
|---|---|
| **F0 Fundament i spike'i** | Repo, CI, kontrakty jądra, wzorzec modułu, `ui-kit` v0, ADR-y i time-boxowane spike'i (głos, most CLI, RAM Tauri, sprzęt, dane, Broker-UI). |
| **F1 Rdzeń czatu** | Czat z dostawcami API i lokalnym llama.cpp, sesje, hub kont i kluczy, ustawienia, zasobnik, Szybkie pytanie, eksport `.alfa` — bez narzędzi agentek. |
| **F2 Głos rdzeniowy + agentki** | Pełny potok głosowy z barge-in, cztery persony z głosami v0 bez kluczy, obsada ról, panele Głos i Agentki. |
| **F3 Safety Kernel + system** | Broker, Broker-UI, watchdog, audyt, cofanie, kill-switch, poziomy L0–L4, pierwsze narzędzia fs/shell/schowek — **koniec MVP**. |
| **F4 Mosty i MCP** (∥ F5) | Claude Code i Codex jako „opaque worker", karty zgodności, wbudowany terminal, klient i serwer MCP. |
| **F5 Agentki i głos rozszerzony** (∥ F4) | Wiele agentek równolegle, Scheduler, Marszałek, Kreator, wyzwalacze; słowa wywoławcze, weryfikacja mówcy, S2S, dyktowanie, czytanie na głos, pigułka głosowa. |
| **F6 Computer use** | UIA, wizja, wejście, przeglądarka, Office, helper UAC, panel Ekran; wymaga klucza API lub mostu jako „mózgu". |
| **F7 Pamięć pełna + transfer pełny** | Cztery warstwy pamięci, konsolidacja, Inspektor, kaskadowe `forget`, kopie zapasowe. |
| **F8 Samonaprawa i ulepszanie** | Diagnosta, Ulepszacz z bramką ewaluacyjną i holdoutem, wtyczki Wasm, „Zdrowie systemu". |
| **F9 Dopieszczenie** | Audyt a11y, wydajność i bateria, pentest, dokumentacja, „Co nowego". |

**MVP = F0–F3**: działa bez żadnych kluczy na baseline (lokalny model, lokalny głos, Broker, cofanie, kill-switch, `.alfa`).

## Jak zacząć

Pierwsze sesje AI (każda w osobnym worktree; szczegóły i lista bramek w `docs/AI_WORKFLOW.md` §9–10):

1. Człowiek: przygotowanie maszyn (toolchainy Rust MSVC, Node, Vulkan SDK, CUDA, sterowniki, wirtualny kabel audio), repo prywatne, self-hosted runnery na osobnym koncie Windows, logowanie do Claude Code i Codex.
2. *(Opus 5.5, recenzja GPT-6 Sol)* Workspace Rust + `apps/desktop` (Tauri 2 + Svelte 5 „hello"), hooki, CI `windows-latest` zielone na pustym projekcie.
3. *(Opus 5.5, recenzja GPT-6 Sol)* Kontrakty jądra `core-*`, schemat `module.toml`, `SystemPort`-contract z fake'iem, pierwszy moduł-przykład jako wzorzec, szkielet `evals/` i `voice-lab`.
4. *(GPT-6 Sol, recenzja Opus 5.5)* `packages/ui-kit` v0: tokeny, komponenty, Storybook, makiety 1–3 → akceptacja przez człowieka.
5. Spike RAM/startu Tauri → wstępne budżety.
6. Spike pętli głosowej z barge-in na desktopie i laptopie; równolegle nagranie korpusu własnego przez człowieka.
7. Spike'i sprzętowe, kandydaci głosu, dane (SQLCipher + sqlite-vec + FTS5), Broker-UI → ADR-y do akceptacji.

## Licencja

Do ustalenia.
