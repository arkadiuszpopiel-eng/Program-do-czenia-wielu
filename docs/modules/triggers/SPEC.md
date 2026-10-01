# triggers — SPEC (v1: kontrakt zaimplementowany)

## Cel
Wyzwalacze tworzące zadania dla schedulera (PLAN §9.5, ARCHITECTURE: „harmonogram i wyzwalacze”): czasowe (cron w strefie z DST, jednorazowe, interwały), zdarzeniowe (plik w katalogu, nowa wiadomość, koniec zadania) i ręczne. **Nigdy nie uruchamiają mostu CLI** (PLAN §1.3 pkt 4, `docs/compliance/subscription-routes.md` §2.4).

## Fala i priorytet
F5, P1. Fokus okna, schowek, e-mail — kolejne rodzaje po F6 (porty platformy).

## Kontrakt (źródło prawdy: `crates/triggers-contract`)
```rust
pub struct TriggerSpec { id, name, owner: Actor /* User|Agent|System = twórca */, kind: Cron{CronExpr}|Once{at_ms}|Interval{every_ms}|FileInDir{dir, pattern}|NewMessage{session}|TaskFinished{task_prefix, outcome}|Manual,
  tz: Tz /* domyślnie Europe/Warsaw */, action: TriggerAction /* → TaskSpec */, scope: Vec<Capability> /* sufit */, rate: RateLimit, quiet: Option<QuietHours>, quiet_mode: Defer|Skip,
  respect_dnd, misfire: FireOnce|Skip, allow_bridges, enabled }
pub trait Triggers { create, update, remove, set_enabled, fire_now, input(TriggerInput), set_dnd, list, get, log }
```
Rdzeń `TriggerEngine` + `TriggersCore<H: TriggerHost>` w kontrakcie. Zdarzenia: `triggers.trigger.{created,updated,removed,toggled}`, `triggers.{fired,suppressed,deferred,failed}` (bez celu akcji; z pliku tylko nazwa).

## Zależności
`scheduler` (zadania, `Scheduler::submit`), `safety-broker` (`Capability`, `TaintSource`), `personas`, `core-bus`, `core-registry` (`-contract`).

## Niezmienniki
- Zadanie z wyzwalacza ma pochodzenie `Trigger{depth}` → most odmawia (`LaunchOrigin::Trigger`); akcja z mostem bez `allow_bridges` odrzucana przy tworzeniu. Wyjątek: harmonogram **czasowy** utworzony i posiadany przez **użytkownika** z `allow_bridges` i limitem ≤ 24/dobę → `Schedule` (zgodę per trasa i limit sprawdza jeszcze `agent-backends`).
- Właściciel = twórca; agentka zarządza tylko swoimi; klasa zadania najwyżej `Agent`.
- `scope` to sufit — tokeny wydaje Broker przy wykonaniu, nigdy przy tworzeniu.
- Treść wyzwalająca (plik, wiadomość, wynik skażonego zadania) jest niezaufana: `taint` + osobne `payload.untrusted`.
- Cron: godzina nieistniejąca (przeskok) → pierwsza chwila po przeskoku; powtórzona → tylko pierwsze wystąpienie; każda minuta lokalna najwyżej raz.
- Limity: per wyzwalacz i globalny 120/h; łańcuch wyzwalaczy ≤ 3, własne zadania ignorowane; cisza/DND odkłada (jedno uruchomienie zbiorcze) albo pomija; dziennik uruchomień (100/wyzwalacz, 1000 łącznie).

## Zdolności / uprawnienia
Brak (obserwacja katalogów przez port platformy).

## Izolacja
`inproc`, `always`.

## Budżet zasobów
RAM ≤ 4 MB (≤ 512 wyzwalaczy); wyliczenie następnego wystąpienia ≤ 1 ms (horyzont 8 lat).

## Konfiguracja (klucze TOML)
`[triggers] state_path`, `global_max_fires_per_hour = 120` (polityka), domyślna strefa `tz = "Europe/Warsaw"`.

## Wkład do UI
Ustawienia → Wyzwalacze (lista, następne uruchomienie, dziennik, „Uruchom teraz”, cisza), Kreator agentów (`triggers: [{cron}]`), karta zgodności przy `allow_bridges`.

## Testy akceptacyjne
- `ACC-F5-triggers-01` (F5-04): „most nie startuje z wyzwalacza” 0/100 — `triggers-impl/tests/compliance.rs`, `evals/F5/bridge-trigger-cases.json`.
- `ACC-F5-triggers-02`: cron/DST — właściwości (`triggers-fake/tests/cron_props.rs`) + kontrakt (wiosna/jesień 2026 w Warszawie).

## Fake
`triggers-fake`: wirtualny zegar, nagrane zadania i zdarzenia, odmowa schedulera, restart, przeskok bez wyzwalania.

## Otwarte pytania
- Obserwator plików w `platform-windows` (`ReadDirectoryChangesW`) — port gotowy, podpięcie w `app-*`.
- `TaintSource` nie ma wariantu „wiadomość czatu” — używany `Email` (do decyzji z właścicielem Brokera).
