# Mapa dokumentacji Alfy

Każdy dokument ma jedno miejsce i jedno zadanie. Jeśli nie wiesz, gdzie coś jest — zacznij tutaj.

## Od czego zacząć

| Kim jesteś | Czytaj w tej kolejności |
| --- | --- |
| Sesja AI | [`../AGENTS.md`](../AGENTS.md) → [`STATUS.md`](STATUS.md) → [`KONTYNUACJA.md`](KONTYNUACJA.md) → SPEC modułu, nad którym pracujesz |
| Właściciel — co dalej | [`html/index.html`](html/index.html) (plan i postęp w przeglądarce) → [`KONTYNUACJA.md`](KONTYNUACJA.md) (rozdz. 8: decyzje) → [`STATUS.md`](STATUS.md) („Co musi zrobić człowiek”) |
| Właściciel — jak używać programu | [`user-guide/`](user-guide/README.md) |

## Plan i stan

| Dokument | Co zawiera | Kto zmienia |
| --- | --- | --- |
| [`PLAN.md`](PLAN.md) | Zatwierdzony plan programu (wersja 5): decyzje, architektura, fale F0–F9, ryzyka. Źródło prawdy o tym, **co** budujemy | zmiany tylko przez ADR albo za zgodą właściciela |
| [`plan/`](plan/README.md) | **Plan v6** — rozszerzenie planu: stan wykonania, architektura docelowa, roadmapa etapów A–H, nowe funkcje, Windows / Linux / panel WWW / Android, dostęp zdalny, zgodność prawna, technologie | sesja AI po każdym zadaniu i etapie; decyzje — właściciel |
| [`html/`](html/index.html) | Ten sam plan, stan, dziennik zmian i ADR-y jako strony HTML z paskiem postępu (`node scripts/docs-html.mjs`) | generowany — nie edytować |
| [`STATUS.md`](STATUS.md) | Macierz 108 kryteriów akceptacji ze stanem i dowodem, luki techniczne, lista bramek ludzkich | sesja AI po każdej sesji |
| [`../CHANGELOG.md`](../CHANGELOG.md) | Dziennik zmian po ludzku: każda seria poprawek = wyższy numer wersji i wpis | sesja AI przy podbiciu wersji |
| [`KONTYNUACJA.md`](KONTYNUACJA.md) | Ocena stanu z 2026-10-07, etapy dalszych prac, decyzje właściciela | sesja AI; decyzje — właściciel |
| [`ACCEPTANCE.md`](ACCEPTANCE.md) | Definicje kryteriów i progi fal, definicja MVP | progi — tylko właściciel |
| [`RELEASE.md`](RELEASE.md) | Jak powstaje wydanie i instalator | sesja AI |

## Architektura i decyzje

| Dokument | Co zawiera |
| --- | --- |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Mikrojądro, magistrala, moduły, procesy, przepływy |
| [`CATALOG.md`](CATALOG.md) | Generowany katalog wszystkich crate'ów i modułów: grupa, fala, rodzaj, rozmiar, testy, SPEC, niespójności (`node scripts/catalog.mjs`) |
| [`modules/`](modules/README.md) | Jednostronicowe specyfikacje modułów (`<moduł>/SPEC.md`) i ich indeks |
| [`ADR/`](ADR/README.md) | Rejestr decyzji architektonicznych 0001–0016 oraz projekty 0017–0025 do planu v6 (czekają na podpis właściciela) |
| [`VOICE.md`](VOICE.md) | Voice Suite: potok, profile A–D, barge-in |
| [`PERSONAS.md`](PERSONAS.md) | Cztery agentki, role, obsady |
| [`UI.md`](UI.md) | Interfejs: okna, panele, tokeny, budżety |
| [`formats/`](formats/alfa-package.md) | Format paczki `.alfa` |
| [`vendor/`](vendor/README.md) | Notatki o zależnościach nowszych niż wiedza modeli (Tauri, wasmtime, wasapi…) |

## Bezpieczeństwo i jakość

| Dokument | Co zawiera |
| --- | --- |
| [`THREAT_MODEL.md`](THREAT_MODEL.md) | Aktywa, granice zaufania, scenariusze ataku, twarde blokady |
| [`reviews/`](reviews/) | Przeglądy bezpieczeństwa #1–#3, przegląd fali 3, audyt interfejsu, przegląd zmian 0.0.1–0.0.5 — z listą ustaleń otwartych |
| [`compliance/`](compliance/subscription-routes.md) | Zgodność tras abonamentowych, rejestr tras |
| [`AI_WORKFLOW.md`](AI_WORKFLOW.md) | Jak pracują sesje AI: kolejność pracy nad modułem, recenzja, Definition of Done, bramki ludzkie |

## Poza `docs/`

| Miejsce | Co zawiera |
| --- | --- |
| [`../README.md`](../README.md) | Wizytówka projektu, stan w jednym akapicie, jak zacząć |
| [`../AGENTS.md`](../AGENTS.md) | Reguły dla sesji AI (≤ 120 linii; zmiany zatwierdza właściciel) |
| [`../crates/README.md`](../crates/README.md) | Zasada trójki crate'ów i wyjątki `lib-*`, `app-*` |
| [`../scripts/README.md`](../scripts/README.md) | Skrypty: bramki, instalacja, środowisko przenośne |
| [`../evals/README.md`](../evals/README.md) | Zestawy akceptacyjne, spike'i, zasada zamrażania |
| [`../providers-catalog/README.md`](../providers-catalog/README.md) | Katalog dostawców modeli |
| [`../apps/desktop/README.md`](../apps/desktop/README.md) | Powłoka Tauri i interfejs |

## Zasady porządku

- Nowy dokument trafia do istniejącej kategorii powyżej i dostaje wiersz w tej mapie.
- Liczby o kodzie (crate'y, linie, testy) bierzemy z `CATALOG.md`, nie przepisujemy ręcznie.
- Stan kryteriów zmienia się tylko w `STATUS.md`; inne dokumenty do niego odsyłają.
- Pliki generowane (`CATALOG.md`, `html/`, schematy, tokeny) nie są edytowane ręcznie.
- Po zakończonym zadaniu: stan w `plan/03-roadmapa.md` (i w `STATUS.md`, jeśli zamyka kryterium). Po zakończonym
  etapie: `plan/00-stan-wykonania.md`, wyższa wersja z wpisem w `CHANGELOG.md`, potem `node scripts/docs-html.mjs`.
