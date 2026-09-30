# AI_WORKFLOW.md — jak Alfa jest budowana w 100% przez AI

Dokument operacyjny wynikający z `docs/PLAN.md` §4 (wykonawcy, środowisko, procedura, bramki), §16.2 (fale) i §18 (weryfikacja). Reguły dla agentów w skrócie są w `AGENTS.md`; tutaj jest pełny opis procesu, listy kontrolne i szablony.

## 1. Zasada podstawowa

Cały kod, testy, dokumentacja i infrastruktura CI powstają w sesjach modeli AI. Człowiek (właściciel projektu) nie pisze kodu: uruchamia sesje, zatwierdza SPEC-i Jądra/Brokera, ADR-y, makiety i PR-y do Jądra/Brokera, wykonuje bramki ludzkie (§9) i decyduje o kierunku. Jakość gwarantuje nie deklaracja, lecz mierzalne bramki (CI, Definition of Done, zamrożone zestawy akceptacyjne) — wyniki bramek są widoczne w każdym PR.

## 2. Wykonawcy i podział pracy

| Wykonawca | Model | Domyślny zakres | Uwagi |
|---|---|---|---|
| Claude Code | `claude-opus-5-5` | Rust: jądro (`core-*`), Broker, Voice Suite, `platform-windows`, narzędzia systemowe | Logowanie własnym planem właściciela (zgodne z §1.3 planu) |
| Codex CLI | GPT-6 Sol | UI/Svelte 5, `ui-kit`, testy E2E (Playwright), dokumentacja, Storybook | Zgłaszano przycinanie kontekstu → moduł musi mieścić się w małym oknie |

- Podział można odwrócić per moduł. Nadrzędna jest zasada niżej: **kto napisał, ten nie recenzuje**.
- Oba narzędzia działają przez własne logowanie właściciela; Alfa (program) nie ma z tym nic wspólnego — to sesje deweloperskie, uruchamiane przez człowieka, na jego zasadach uprawnień (domyślnie bez dostępu poza katalog repo/worktree).

### 2.1 Przegląd krzyżowy

| Zasada | Szczegół |
|---|---|
| Recenzent = drugi model | Moduł napisany przez Opus recenzuje GPT-6 Sol i odwrotnie. Inna rodzina modeli łapie skorelowane ślepe punkty. |
| Świeży kontekst | Recenzja odbywa się w nowej sesji, bez historii sesji autora. Wejście: diff PR, SPEC, kontrakt, wyniki CI. |
| Zakres recenzji | Wyłącznie: poprawność, bezpieczeństwo, zgodność ze SPEC i kontraktem, spełnienie DoD. Nie „znajdź cokolwiek", nie styl. |
| SPEC | **Pisze autor, akceptuje recenzent.** SPEC-i Jądra i Brokera akceptuje dodatkowo człowiek. |
| Zestawy akceptacyjne fal | **Tworzy model-recenzent** (nie autor), akceptuje człowiek, potem **zamrożone hashem w `evals/`** przed rozpoczęciem implementacji. Powód: autor nie może pisać testów pod własny kod. |
| Zamknięcie uwag | Wszystkie uwagi recenzenta zamknięte (poprawka albo uzasadnione odrzucenie zaakceptowane przez recenzenta) przed merge. |

### 2.2 Backlog i sesje

- **Backlog = GitHub Issues**, w kolejności fal z §16.2 planu. Każde Issue to jedna „karta zadania" (szablon w §11).
- **Jedna karta = jedna sesja = jeden moduł = jeden git worktree.** Granice modułów (trójka crate'ów) zapobiegają konfliktom między równoległymi sesjami.
- Sesja nie dotyka plików innych modułów. Jeśli potrzebuje zmiany w cudzym kontrakcie — zakłada osobne Issue.
- Kontekst modelu nie jest gwarantowany, dlatego moduł musi być w całości „obejmowalny": SPEC 1 strona, crate ≤ 8 000 linii, plik ≤ 400 linii.

## 3. Środowisko

| Miejsce | Do czego | Ograniczenia |
|---|---|---|
| **Desktop właściciela** (Ryzen 7 5700X3D · RX 9070 XT 16 GB · 32 GB) | Główna maszyna dev; self-hosted runner ścieżki **AMD/Vulkan**; emulacja baseline (limity 6 rdzeni / 16 GB / VRAM 8 GB, korekta GPU ×2,2) | Nie ma RDNA3 — luka opisana w §3.5 planu |
| **Laptop właściciela** (i7-13700H · RTX 4050 6 GB · 16 GB) | Self-hosted runner ścieżki **CUDA**; testy ciasnego VRAM, baterii, wbudowanego mikrofonu/głośników (AEC) | 6 GB VRAM < baseline |
| **Chmura (sesje Claude Code / Codex w kontenerze)** | Moduły niezależne od sprzętu: jądro, scheduler, pamięć, router, UI na atrapach | Brak Windows, audio, GPU → fake'i |
| **GitHub Actions `windows-latest`** | **Tylko build, lint i testy jednostkowe** | Minuty Windows liczą się podwójnie w prywatnym repo; pomiary wydajności zaszumione → nie mierzymy tu budżetów |

Reguły:
1. **Self-hosted runner (desktop, emulacja baseline) jest wymaganym checkiem przed merge** dla modułów sprzętowych i wydajnościowych: `voice-*`, `providers-local`, `platform-windows`, `model-residency`, budżety UI (§14.7 planu). Bez niego DoD pkt 4 nie da się spełnić.
2. **Nightly:** pełna macierz baseline (emulowany) × desktop-AMD × laptop-CUDA (zasilanie i bateria) + walidacja adapterów API na żywo (gdy są klucze) + ręczny nightly kontraktów mostów.
3. **Bezpieczeństwo runnera:** kod pisany przez AI i automatycznie mergowany wykonuje się na maszynach właściciela → runner działa na **osobnym koncie Windows bez dostępu do profilu właściciela**, repo jest prywatne, PR-y z forków są wyłączone, sekrety runnera minimalne.
4. Maszyny przygotowuje człowiek (bramka #8): Rust MSVC, VS Build Tools, Node, Vulkan SDK, CUDA, przypięte sterowniki, wirtualny kabel audio, VM (Hyper-V przy Win11 Pro).

## 4. Pliki sterujące

| Plik | Rola | Kto pisze / zatwierdza |
|---|---|---|
| `AGENTS.md` (≤ ~100 linii) | Reguły niewyprowadzalne z kodu; egzekucja hookami i CI, nie prozą. Codex przycina AGENTS.md powyżej 32 KiB. | AI pisze (bez `/init`), człowiek zatwierdza |
| `CLAUDE.md` | Cienki plik z `@AGENTS.md` (na Windows bez symlinku) | jw. |
| `docs/modules/<m>/SPEC.md` (1 strona) | Cel, kontrakt, niezmienniki, testy akceptacyjne, budżety modułu | autor / recenzent (Jądro, Broker: człowiek) |
| `docs/vendor/<crate>.md` (≤ 2 strony) | Dokładnie używane API zależności, zweryfikowane kompilacją. Powód: wiedza modeli kończy się w IV–VI 2026 (WASI 0.3, wasmtime 46, Tauri 3 alpha, windows-rs 0.100, tauri-specta RC są dla nich świeże) | autor modułu, który pierwszy używa crate'a |
| `docs/ADR/NNNN-*.md` | Decyzje odwracalne do końca F0 | AI proponuje, człowiek akceptuje |
| `evals/` | Zamrożone zestawy akceptacyjne fal (hash), zadania wzorcowe, benchmarki głosu, holdout (F8) | recenzent tworzy, człowiek akceptuje |
| `.github/workflows/*` | CI: bramki z §6 | AI (F0), zmiany przez człowieka |

Zasada zależności: przypinaj wersje, które modele znają, albo dawaj im dokumentację (`docs/vendor`, docs.rs). Nowy crate = wpis w `Cargo.lock` sprawdzony `cargo deny` i przegląd (halucynowane crate'y/API).

## 5. Kolejność pracy nad modułem (lista kontrolna sesji)

| Krok | Co | Gotowe, gdy |
|---|---|---|
| 0 | Przeczytaj kartę zadania (Issue), `AGENTS.md`, `docs/PLAN.md` w sekcjach wskazanych w karcie, `docs/ARCHITECTURE.md`, SPEC-i modułów, od których zależysz | Rozumiesz kontrakt wejścia/wyjścia i budżety |
| 1 | Załóż worktree `git worktree add ../wt-<m> -b feat/<m>` | Praca poza `main` |
| 2 | **SPEC** — `docs/modules/<m>/SPEC.md`: cel, kontrakt, niezmienniki, testy akceptacyjne, budżety RAM/CPU/opóźnień | Recenzent zaakceptował SPEC (komentarz w Issue) |
| 3 | **Kontrakt** — `crates/<m>-contract`: trait, typy, zdarzenia, JSON Schema, generowane typy TS | `cargo build`, schema w `packages/schemas`, `git diff --exit-code` na generowanych |
| 4 | **Fake** — `crates/<m>-fake`: deterministyczna atrapa (wirtualny zegar, record/replay) | Fake przechodzi testy kontraktowe |
| 5 | **Testy najpierw** — kontraktowe (parametryzowane fake/impl), jednostkowe, property-based, stany brzegowe (§14.4 planu) | Testy czerwone na pustym impl |
| 6 | **Implementacja** — `crates/<m>-impl`, `module.toml`, strona ustawień z manifestu, dokumentacja użytkownika | Testy zielone, zero ostrzeżeń, zero `unwrap()` w produkcji |
| 7 | **Pomiar budżetów** na baseline emulowanym (self-hosted) — dla modułów sprzętowych obowiązkowo | Wyniki w PR, w granicach SPEC |
| 8 | **Lista bezpieczeństwa** (uprawnienia, wejście niezaufane, sekrety, deny-listy) | Sekcja wypełniona w PR |
| 9 | PR wg szablonu (§12), CI zielone | Wszystkie checki zielone |
| 10 | **Przegląd drugiego modelu** w świeżym kontekście | Uwagi zamknięte |
| 11 | **Merge** — automerge; Jądro/Broker/polityki: człowiek | Issue zamknięte, worktree usunięty |

## 6. Heurystyki rozmiaru i bramki CI

**Heurystyki rozmiaru** (egzekwowane narzędziami; liczby to heurystyka, nie wynik badań):

| Jednostka | Limit | Narzędzie |
|---|---|---|
| Plik | ≤ 300–400 linii | lint w hooku pre-commit |
| Crate | ≤ 5–8 tys. linii | skrypt CI |
| Funkcja | clippy `too_many_lines` | clippy |
| Graf zależności | moduł → tylko `-contract` innych modułów | `cargo-deny` + skrypt grafu; `dependency-cruiser` dla TS |

**Bramki CI** (każda blokuje merge):

| Bramka | Zakres |
|---|---|
| `cargo fmt --check`, `clippy -D warnings` | cały workspace |
| `cargo test` | testy jednostkowe, kontraktowe, property-based, regresyjne |
| `cargo deny` / `cargo vet` + przegląd `Cargo.lock` przy nowym crate | łańcuch dostaw, halucynowane crate'y |
| Lint „zero `TODO/FIXME/unimplemented!/unwrap()`" w ścieżkach produkcyjnych | Rust |
| `svelte-check` + `eslint-plugin-svelte` (zakaz `export let`, `$:`, `on:`) | UI |
| `git diff --exit-code` na plikach generowanych (typy TS, schematy) | kontrakty |
| axe (a11y): 0 naruszeń critical/serious | Storybook / E2E |
| E2E: Playwright przez CDP do WebView2 (tylko build testowy) albo `tauri-driver` | UI |
| Eval-e (fixture'y syntetyczne adapterów, evals narzędzi na modelu lokalnym) | modele |
| Budżety lekkości (§3.4) i opóźnień (§6.4) na baseline emulowanym | self-hosted, wymagany check dla modułów sprzętowych |
| Test reguły skrótów `Ctrl+Alt+litera` (AltGr), test zamkniętego portu CDP w produkcji, test „zero fontów webowych / `backdrop-filter`" | reguły twarde |
| Pokrycie: ≥ 85% linii, ≥ 70% gałęzi dla modułów nie-UI (wstępnie; do ustalenia w F0) | Rust |

## 7. Definition of Done modułu

Bez spełnienia wszystkich punktów — brak merge.

1. `SPEC.md` aktualny; kontrakt + `-fake` + testy kontraktowe zielone.
2. Testy jednostkowe i właściwościowe dla logiki; pokrycie ≥ 85% linii / ≥ 70% gałęzi (moduły nie-UI, wstępnie); test regresyjny dla każdego naprawionego błędu.
3. Zero `TODO/FIXME/unimplemented!/unwrap()` w ścieżkach produkcyjnych, zero ostrzeżeń.
4. Budżety zasobów modułu (RAM/CPU/opóźnienie) zmierzone na **baseline emulowanym** i spełnione (F0–F1: progi wstępne, zaostrzane po pomiarach F0).
5. Obsługa błędów i stany brzegowe (§14.4 planu) pokryte testami; logi/zdarzenia zgodne ze schematem (§13 planu).
6. Przegląd drugiego modelu w świeżym kontekście — wszystkie uwagi zamknięte.
7. Lista kontrolna bezpieczeństwa (uprawnienia, wejście niezaufane, sekrety) i dokumentacja użytkownika/ustawień.
8. Merge tylko na zielono; moduły Jądra/Brokera i zmiany polityk bezpieczeństwa dodatkowo przez człowieka.

## 8. Symulatory i fake'i

Bez nich AI nie może weryfikować pracy bez człowieka i sprzętu. Powstają w F0 jako wzorzec i są utrzymywane razem z kontraktami.

| Fake | Co symuluje | Użycie |
|---|---|---|
| Fake audio | odtwarzanie WAV do wejścia, przechwycenie wyjścia, **wirtualny zegar** | potok głosowy testowany deterministycznie; p50/p95 liczone z zegara |
| Fake LLM | record/replay odpowiedzi i wywołań narzędzi z fixture'ów | testy routera, agent-runtime, UI bez kluczy |
| Fake UIA | drzewa z fixture'ów (JSON) per aplikacja | `tools-uia`, macierz aplikacja × trasa |
| Fake `SystemPort` | wirtualny system plików, procesy, schowek, rejestr | wszystkie moduły `tools-*`, `platform-windows`-contract |
| Fake dostawca STT/TTS | deterministyczne transkrypty i audio | `voice-*`, Voice Lab |
| Fake Broker | wydawanie tokenów zgodnie z polityką testową | testy uprawnień poza F3 |

Zasada: każdy `-fake` przechodzi te same testy kontraktowe co `-impl`. Prawdziwy sprzęt/chmura wchodzi dopiero w nightly na self-hosted lub po dodaniu klucza.

## 9. Pierwsze dwa tygodnie (F0) — kto co robi

Każdy punkt to osobna sesja AI z własnym worktree; kolejność jak niżej.

| # | Sesja | Autor / recenzent | Wynik |
|---|---|---|---|
| 1 | Repo: workspace Rust + `apps/desktop` (Tauri 2 + Svelte 5 „hello"), `AGENTS.md`, `CLAUDE.md`, hooki (fmt/clippy/svelte-check), CI `windows-latest` zielone na pustym projekcie, szkic `docs/THREAT_MODEL.md` | Opus 5.5 / GPT-6 Sol | Zielony pusty projekt |
| 2 | Kontrakty jądra `core-bus/registry/config/log` + schemat `module.toml` + `SystemPort`-contract z fake'iem + pierwszy moduł-przykład z trójką crate'ów i testem kontraktowym (**wzorzec dla wszystkich**); szkielet `evals/` i harnessu `voice-lab` | Opus 5.5 / GPT-6 Sol | Wzorzec modułu |
| 3 | `packages/ui-kit` v0: tokeny (§14.3), podstawowe komponenty, Storybook, makiety 1–3 (start, rozmowa, tryb głosowy) | GPT-6 Sol / Opus 5.5 | **Akceptacja makiet przez człowieka** (bramka #4) |
| 4 | Spike (f): RAM/start Tauri przy 1 i 3 oknach | dowolny / drugi | Wstępne budżety §3.4 i §14.7 |
| 5 | Spike (a): pętla głosowa mic → VAD → whisper.cpp → Pocket-PL → głośnik z barge-in, desktop i laptop; równolegle **nagranie korpusu** przez człowieka (bramka #3) | Opus 5.5 / GPT-6 Sol | go/no-go profilu A |
| 6 | Spike (h) pomiary sprzętowe + spike (e) tabela kandydatów głosu → ADR (4), (11); spike (i) dane → ADR (8); spike (k) Broker-UI → ADR (3) | Opus 5.5 / GPT-6 Sol | ADR-y do akceptacji |

Równolegle: prace UI ∥ spike'i głosu. Spike'i (c) UIA i (d) współdzielenie wejścia przeniesione na początek F6; spike (g) odroczony do klucza Anthropic.

## 10. Bramki ludzkie (tego AI nie zrobi)

| # | Bramka | Kiedy |
|---|---|---|
| 1 | Logowanie do CLI (Claude Code, Codex) i klucze API | start; klucze w dowolnym momencie |
| 2 | Potwierdzenia UAC i (opcjonalnie) Windows Hello; podniesienie do L4 „Maks" | F3+ |
| 3 | **Nagranie korpusu własnego** (~30–45 min mowy w różnych warunkach) | F0 (równolegle ze spike'iem (a)) |
| 4 | **Akceptacja makiet UI** (§14.10 planu) przed implementacją widoków | F0 (makiety 1–3), potem przed każdą falą |
| 5 | **Odsłuchy:** casting czterech głosów, potwierdzenie „brzmi jak młoda dorosła", wybór końcowy | F0 (e), F2 |
| 6 | Testy akustyczne na desktopie i laptopie (mikrofon/głośniki/słuchawki, wbudowany mikrofon laptopa) | F0, F2, F5 |
| 7 | Decyzje z §19 planu, akceptacja PR-ów Jądra/Brokera, ADR-ów do końca F0, SPEC-ów Jądra/Brokera; uruchamianie sesji | ciągle |
| 8 | **Przygotowanie maszyn:** toolchainy, sterowniki, wirtualny kabel audio, VM | przed F0 |
| 9 | **Repozytorium i runnery:** repo prywatne, self-hosted runnery na osobnym koncie Windows, uprawnienia | przed F0 |
| 10 | **Sekrety infrastruktury:** klucz minisign, lokalny certyfikat podpisu, osobne konto Windows dla usługi Brokera | F0 / F3 |

## 11. Szablon karty zadania (GitHub Issue)

```markdown
# [F<fala>] <moduł> — <jednozdaniowy cel>

**Fala:** F<n> · **Moduł:** `<m>` · **Autor:** Opus 5.5 | GPT-6 Sol · **Recenzent:** drugi model · **Jądro/Broker:** tak | nie

## Cel
<Co ma działać po zamknięciu karty, jednym akapitem.>

## Kontekst do przeczytania
- `docs/PLAN.md` §<…>
- `docs/modules/<m>/SPEC.md` (jeśli istnieje) · `docs/modules/<zależność>/SPEC.md`
- `docs/vendor/<crate>.md`
- `docs/ACCEPTANCE.md` — kryteria F<n>-XX

## Zakres
- [ ] SPEC (jeśli nowy lub zmieniany)
- [ ] `<m>-contract` · [ ] `<m>-fake` · [ ] testy · [ ] `<m>-impl` · [ ] `module.toml`
- [ ] strona ustawień / wkład do UI (jeśli dotyczy)

## Poza zakresem
<Czego nie robić w tej sesji; np. inne moduły, optymalizacje.>

## Kryteria akceptacji (mierzalne)
| ID | Kryterium | Próg | Jak mierzymy |
|---|---|---|---|
| F<n>-XX | … | … | test / self-hosted / eval |

## Budżety modułu
RAM ≤ … · CPU idle ≈ 0 · opóźnienie … (baseline emulowany)

## Bezpieczeństwo
Zdolności żądane: … · Wejście niezaufane: … · Sekrety: … · Odwracalność: `reversible: yes|scoped|no`

## Wymagany runner
`windows-latest` | self-hosted desktop (AMD) | self-hosted laptop (CUDA) | nightly

## Definition of Done
- [ ] 1–8 z `docs/AI_WORKFLOW.md` §7
```

## 12. Szablon PR

```markdown
## Moduł i karta
Zamyka #<issue> · moduł `<m>` · fala F<n> · autor: <model> · recenzent: <model>

## Co się zmienia
<3–6 punktów; co dodano, co zmieniono w kontrakcie (jeśli cokolwiek — wersja kontraktu podbita).>

## Jak zweryfikowano
- Testy: kontraktowe (fake + impl) / jednostkowe / property-based / regresyjne — liczby
- Budżety na baseline emulowanym: RAM … · CPU … · opóźnienie p50/p95 … (link do artefaktu self-hosted)
- Kryteria `docs/ACCEPTANCE.md`: F<n>-XX ✔ / ✘ z wartością

## Bezpieczeństwo
- Zdolności / tokeny: …
- Wejście niezaufane i deny-listy: …
- Sekrety i redakcja w logach: …
- Dotyka Jądra / Brokera / polityk: tak | nie (jeśli tak — wymagany przegląd człowieka)

## Definition of Done
- [ ] SPEC aktualny, kontrakt + fake + testy kontraktowe zielone
- [ ] pokrycie ≥ 85 % / ≥ 70 %, regresja dla błędów
- [ ] zero TODO/FIXME/unimplemented!/unwrap(), zero ostrzeżeń
- [ ] budżety zmierzone na baseline emulowanym
- [ ] stany brzegowe i błędy pokryte; zdarzenia zgodne ze schematem
- [ ] przegląd drugiego modelu — uwagi zamknięte
- [ ] lista bezpieczeństwa + dokumentacja ustawień
- [ ] CI zielone (w tym self-hosted, jeśli wymagany)

## Dla recenzenta (świeży kontekst)
Sprawdź wyłącznie: poprawność, bezpieczeństwo, zgodność ze SPEC/kontraktem, DoD. Pliki generowane pomiń.
```

## 13. Uczciwe zastrzeżenia

- „Idealnie" nie da się zagwarantować; ten proces zamienia je na listę mierzalnych kryteriów i bramek.
- Progi pokrycia i budżety z F0–F1 są wstępne; zaostrzane po pomiarach F0 i tylko przez zmianę SPEC z akceptacją recenzenta.
- Ewaluacje LLM są niedeterministyczne: N ≥ 5 powtórzeń, przedziały ufności, przypięte wersje modeli, budżet kosztów.
