# app-tasks

Zadania, wyzwalacze i Marszałek w aplikacji (kategoria `app-*`).

- `modules::{scheduler, triggers, marshal}` — `scheduler-impl` (zastępuje `scheduler-lite-impl`; ta
  sama tablica blokad dla mowy i zadań, stan `scheduler.json`), `triggers-impl` (`triggers.json`,
  obserwacja katalogów: `watch::PlatformFileWatch` nad `DirWatchPort` z `platform-windows-sys-impl`
  + `watch::spawn_pump` „nowy plik" → `file_created`; bez portu `NoFileWatch`), `marshal-impl`
  (`marshal.json`).
- `LateExecutor` / `AppExecutor` — wykonawczyni: agentka przez `agent-runtime-impl::RuntimeExecutor`
  (hak `StepGate::boundary` w runtime: steering ≤ 1 krok, oddanie, stop), most CLI przez
  `agent-backends` (`TaskOrigin::launch_origin` — wyzwalacz i Ulepszacz zawsze odmowa, harmonogram tylko
  ze zgodą per trasa; sesja „tylko lokalnie" nigdy), usługi — brak.
- `TaskHost` — port do rdzenia (sesja „Zadania w tle", katalog roboczy, prywatność, Replay);
  implementacja w `app-core/src/host.rs`.
- `BrokerSink` / `BridgeRuns` / `BridgeProjector` — prośby o uprawnienia mostu przez Broker (karta
  „czeka na zatwierdzenie" w Replay), zdarzenia mostu → Replay „niezweryfikowane przez Alfę".
- `TasksApp` — komendy `tasks_*`, `triggers_*` (podgląd cron w Europe/Warsaw), `marshal_*`
  (propozycja → podgląd zawężenia → zatwierdzenie w UI; polityka: limit równoległości `RosterCtl`, zakaz
  mostów, budżety zadań użytkownika), `delegate` (delegacja z czatu).
- `spawn_bus_bridge` / `spawn_conditions` — zdarzenia → `TaskUpdated`, `TriggerFired`,
  `MarshalReportReady`, eskalacje → toast; DND z `voice-wake`; bezczynność i tryb gry → warunki okien.
- `TasksApp::create_skill` — uruchomienie umiejętności jako zadanie agentki (`app-skills`).

Ograniczenia: propozycje Marszałka trzymane w pamięci procesu (kontrakt nie ma listy propozycji);
zawężenie tokenów Brokera regułami Marszałka — poza `app-*`. Testy: `tests/clock.rs` (cron na
wirtualnym zegarze), `crates/app-core/tests/{tasks,bridges,spy_work}.rs`.
