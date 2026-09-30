# Specyfikacje modułów (`docs/modules/<moduł>/SPEC.md`)

Każdy moduł programu Alfa ma jedną, jednostronicową specyfikację (SPEC). SPEC pisze autor modułu (model AI), akceptuje recenzent (drugi model); SPEC-i Jądra i Brokera akceptuje właściciel (PLAN §4.1). Kolejność pracy nad modułem: **SPEC → kontrakt → fake → testy → implementacja → przegląd → CI → merge** (PLAN §4.3).

Zasady:
- Moduł = trójka crate'ów `<m>-contract`, `<m>-impl`, `<m>-fake` + manifest `module.toml` (PLAN §3.2). Inne moduły zależą **tylko od `-contract`**.
- SPEC ma mieścić się w małym oknie kontekstu (cel: 40–90 linii). Szczegóły idą do ADR, `docs/vendor/` albo do kodu.
- Szkic Rust w sekcji „Kontrakt" jest **orientacyjny** — źródłem prawdy staje się crate `-contract` po jego powstaniu; SPEC się wtedy aktualizuje (DoD pkt 1).
- Nie wymyślamy decyzji spoza PLAN.md. Braki oznaczamy „do ustalenia w SPEC v1" (albo wskazujemy ADR/spike).
- ID testów akceptacyjnych: `ACC-F<fala>-<moduł>-<nr>` (np. `ACC-F1-sessions-01`). Rejestr ID i progi: `docs/ACCEPTANCE.md`; zestawy zamrażane hashem w `evals/`.

## Szablon SPEC

```markdown
# <moduł> — SPEC (szkic v0)

## Cel
Jedno–trzy zdania: co moduł robi i czego NIE robi.

## Fala i priorytet
Fala (§16.2), priorytet P0/P1/P2, wersje (v0/v1) i co wchodzi w każdą.

## Kontrakt (szkic Rust)
trait główny + 2–5 kluczowych typów + nazwy zdarzeń na magistrali (`<moduł>.<zdarzenie>`).
Oznaczony jako szkic; JSON Schema zdarzeń w `packages/schemas/`.

## Zależności
Tylko crate'y `-contract` innych modułów (+ `core-*`). Zależności zewnętrzne w `docs/vendor/`.

## Niezmienniki
Reguły, których implementacja nigdy nie łamie (testowane property-based / kontraktowo).

## Zdolności / uprawnienia
Tokeny zdolności z Brokera, jakich moduł potrzebuje (`fs.read(zakres)`, `net.egress(host)`…), lub „brak".

## Izolacja
`inproc` | `process` | `wasm` + cykl życia (`always` | `lazy` | `on-demand`) + wątek RT, jeśli dotyczy.

## Budżet zasobów
RAM / CPU / opóźnienie deklarowane w `module.toml` (wstępne; zaostrzane po pomiarach F0 na baseline).

## Konfiguracja (klucze TOML)
Klucze w `%APPDATA%\Alfa\config\*.toml` (wspólne) lub `config/machine/<id>.toml` (per maszyna), z domyślnymi.

## Wkład do UI
Panel / strona ustawień / elementy composera / zasobnik — lub „brak".

## Testy akceptacyjne
Lista ID `ACC-…` z progami (z §16.2 / ACCEPTANCE.md).

## Fake
Co udaje crate `-fake` i jak jest sterowany (fixture'y, wirtualny zegar, record/replay).

## Otwarte pytania
Punkty „do ustalenia w SPEC v1", odwołania do ADR i spike'ów.
```

## Katalog modułów P0 (szkielety SPEC)

| Moduł | Grupa | Fala | Status |
|---|---|---|---|
| `core-bus` | Jądro | F0 | szkic |
| `core-registry` | Jądro | F0 | szkic |
| `core-config` | Jądro | F0 | szkic |
| `core-log` | Jądro | F0 (v1 w F1) | szkic |
| `platform-windows` | System | F1 (v1), F5 (v1.5), F6 (v2) | szkic |
| `sessions` | Dane | F1 | szkic |
| `search` | Dane | F1 | szkic |
| `artifacts` | Dane | F1 | szkic |
| `memory` | Dane | F1 (v0), F7 (pełna) | szkic |
| `accounts-hub` | Modele | F1 | szkic |
| `providers-api` | Modele | F1 | szkic |
| `providers-local` | Modele | F1 | szkic |
| `router` | Modele | F1 (v1) | szkic |
| `cost-meter` | Modele | F1 | szkic |
| `compliance` | Bezpieczeństwo | F1 (v0), F4 (v1) | szkic |
| `transfer` | Dane | F1 (P0-lite), F7 (pełny) | szkic |
| `device-profile` | Sprzęt | F1 | szkic |
| `ui-shell` | UI | F1 | szkic |
| `ui-kit` | UI | F0 (v0), F1 | szkic |
| `ui-quick` | UI | F1 (Szybkie pytanie, zasobnik), F5 (pigułka) | szkic |
| `shell-integration` | System | F1 | szkic |
| `notify` | System | F1 | szkic |
| `voice-audio` | Głos | F2 | szkic |
| `voice-dsp` | Głos | F2 | szkic |
| `voice-vad` | Głos | F2 | szkic |
| `voice-turn` | Głos | F2 | szkic |
| `voice-stt` | Głos | F2 | szkic |
| `voice-tts` | Głos | F2 | szkic |
| `voice-dialog` | Głos | F2 | szkic |
| `voice-persona` | Głos | F2 | szkic |
| `voice-cmd` | Głos | F2 | szkic |
| `voice-wake` | Głos | F2 (v0), F5 (v1) | szkic |
| `model-residency` | Modele | F2 | szkic |
| `scheduler-lite` | Agentki | F2 (pełny `scheduler` w F5) | szkic |
| `personas` | Agentki | F2 | szkic |
| `agent-runtime` | Agentki | F3 (v0), F5 (v1) | szkic |
| `safety-broker` | Bezpieczeństwo | F3 | szkic |
| `broker-ui` | Bezpieczeństwo | F3 (spike k w F0) | szkic |
| `watchdog` | Bezpieczeństwo | F3 | szkic |
| `updater` | Jądro | F1 (launcher), F3 (aktualizacje, rollback) | szkic |
| `undo-journal` | Bezpieczeństwo | F3 | szkic |
| `risk-classifier` | Bezpieczeństwo | F3 | szkic |
| `tools-fs` | System | F3 | szkic |
| `tools-shell` | System | F3 | szkic |
| `tools-clipboard` | System | F3 | szkic |

Moduły P1/P2 (`agent-backends`, `mcp`, `scheduler`, `marshal`, `agent-builder`, `triggers`, `tools-uia/vision/input/window/system/net/media/browser/office`, `voice-speaker/dictation/readaloud/lab/s2s/transcribe`, `diagnostician`, `improver`, `evals`, `plugin-runtime`, `ui-terminal`) dostają SPEC przed falą, w której powstają.
