# watchdog — SPEC (v1: logika — część 1; proces `alfa-watchdog` i hook skrótu — część 2)

## Cel
Osobny proces nadzoru: heartbeat jądra i modułów `process`, restart modułów z limitem, **safe-mode** po pętli awarii, rollback do ostatniej dobrej wersji/konfiguracji (`updater`, `core-config`), kill-switch (skrót, zasobnik) i zabijanie drzew procesów przez Job Objects — poza UI, < 200 ms (PLAN §8.6, §12.2, §3.1).

## Fala i priorytet
F3. P0. Diagnosta — F8, osobny moduł.

## Kontrakt (źródło prawdy: `crates/watchdog-contract`)
```rust
pub struct Heartbeat { pub from: ProcessRole /* Core|Sidecar(id)|BrokerUi|Broker|CliBridge(id)|Tool(id) */, pub health: Health /* Ok|Degraded|Failing */ }
pub enum WatchAction { Restart{role, attempt}, EnterSafeMode{reason}, StopForSafeMode{role}, LeaveSafeMode, RollbackConfig{revision}, RollbackVersion{to}, RollbackSkipped{reason} }
pub struct WatchPolicy { heartbeat_timeout_ms, max_restarts, window_ms, cooldown_ms, safe_mode_after_crash_loop, auto_rollback }
pub trait Watchdog: KillSwitch + JobRegistry {
    fn watch(&self, role, critical: bool); fn heartbeat(&self, hb) -> Result<Vec<WatchAction>, WatchdogError>;
    fn report_crash(&self, role, detail) -> Vec<WatchAction>; fn tick(&self) -> Vec<WatchAction>;   // wirtualny zegar
    fn mark_last_good(&self); fn safe_mode(&self) -> Option<SafeModeState>; fn may_start(&self, role) -> bool;
    fn leave_safe_mode(&self, ManualConfirmation) -> Result<(), WatchdogError>; fn action_log(&self) -> Vec<WatchAction>;
}
// wspólne z Brokerem: KillSwitch::kill_all(KillReason) -> KillReport, JobRegistry, JobTable, Clock/ManualClock
// porty: Supervisor (restart/stop), ConfigHistory (core-config), UpdaterSignal (updater)
```
Zdarzenia (Diagnostyka; safe-mode/rollback/kill także Audyt przez Brokera): `watchdog.heartbeat.missed`, `watchdog.restart`, `watchdog.crash_loop`, `watchdog.safe_mode.entered/left`, `watchdog.rollback`, `watchdog.kill_switch {latency_us}`, `kernel.audio.silence`.

## Proces (część 2: `watchdog-impl::daemon`, binarka `alfa-watchdog` w `app-safety`)
- `KillSwitchDaemon`: zdarzenia `HotkeyPort` (kill-switch `Ctrl+Shift+F12` rejestrowany wyłącznie przez `WinHotkeys::register_kill_switch`; `RegisterHotKey` + `WH_KEYBOARD_LL`) → `kill_all(Hotkey)`, odbicie 300 ms.
- `ThreadedPeer`: Broker jako peer wołany blokująco (IPC `KillAll`, rola `Watchdog` przyjmowana przez Brokera po tożsamości obrazu `alfa-watchdog.exe`, konto serwera sprawdzane) na osobnym wątku — limit 100 ms działa także przy zawieszonym Brokerze; kolejność: cisza audio → drzewa procesów (od razu) → Broker → Audyt.
- `alfa-watchdog [--broker-pipe P] [--broker-user SID] [-- <jądro> …]`: jądro uruchamiane w Job Object watchdoga (potomkowie dziedziczą zadanie — `TerminateJobObject` zabija całe drzewo, także zagnieżdżone zadania narzędzi). Ikona zasobnika i heartbeat przez IPC — następna iteracja (kill-switch z zasobnika obsługuje dziś powłoka Tauri → Broker).

## Watchdog uruchamiany przez aplikację (F3 część 3, `app-broker`; przegląd CX-c)
- Aplikacja przy starcie uruchamia `alfa-watchdog` z katalogu wersji: usługa — `--broker-pipe P --broker-user SID --lifeline`, tryb przenośny — `--broker-pipe alfa-broker-dev --broker-pid PID --lifeline` (serwer potoku musi być procesem Brokera uruchomionym przez aplikację; po ponownym uruchomieniu Brokera PID się zmienia — wtedy `KillAll` wykonuje aplikacja po komunikacie, patrz niżej).
- `--lifeline`: watchdog kończy się, gdy aplikacja zamknie jego stdin (także po awarii aplikacji — bez sierot).
- **Komunikaty na stdout** (jedna linia JSON, anonimowy potok — czyta tylko aplikacja): `{"event":"ready"}` po zarejestrowaniu skrótu, `{"event":"kill_switch","reason":…,"tokens_revoked":…,"jobs_killed":…,"latency_us":…}` po kill-switchu (`app_safety::watchdog::notice_*`, parser `app_broker::notice`, test zgodności `app-safety/tests/app_launch.rs`).
- **Własność skrótu**: `Ctrl+Shift+F12` rejestruje i obsługuje watchdog (cisza audio → drzewa → Broker `KillAll`); aplikacja **nie** rejestruje skrótu, gdy watchdog zgłosił gotowość, a po komunikacie `kill_switch` wykonuje „STOP WSZYSTKIEGO” w procesie (generacje, przebiegi, scheduler, mowa, drzewa narzędzi, `KillAll`).
- **Zachowanie awaryjne**: brak `alfa-watchdog` obok aplikacji, brak gotowości w 2 s albo koniec procesu watchdoga → aplikacja rejestruje `Ctrl+Shift+F12` sama (dotychczasowa ścieżka `system_kill_all`), stan `watchdog: false` → baner „Watchdog nie działa — STOP WSZYSTKIEGO obsługuje awaryjnie aplikacja” i wpis w Ustawieniach → Uprawnienia. Watchdog nie jest uruchamiany ponownie automatycznie (skrót jest wtedy zajęty przez aplikację) — wraca po ponownym uruchomieniu aplikacji.

## Zależności
`core-bus-contract`, `core-log-contract` (`AuditWriter` Brokera), `platform-contract` (`ProcessPort`). Brak zależności od `safety-broker-contract` — Broker jest peerem `KillSwitch` (działa, gdy Brokera brak).

## Niezmienniki
- Watchdog nie zależy od jądra, UI ani modeli; startuje pierwszy i przeżywa awarię jądra.
- Kill-switch: cisza audio → zabicie wszystkich drzew → peer Brokera (limit 100 ms) → Audyt (limit 50 ms); nie wymaga zatwierdzenia, nie blokuje go brak ani zawieszenie Brokera; błąd jednego drzewa nie zatrzymuje pozostałych (raport, nigdy cicho).
- Restarty: `max_restarts` w `window`; procesy niekrytyczne potem nie są restartowane → safe-mode (jądro, Broker, Broker-UI i procesy krytyczne działają; reszta zatrzymana; heartbeat niekrytycznego w safe-mode → zatrzymanie); procesy krytyczne restartowane zawsze. Wyjście z safe-mode ręczne.
- Rollback tylko do „ostatniej dobrej” (`mark_last_good` po zdrowym starcie), nigdy w pętli (cooldown) i tylko, gdy bieżąca różni się od dobrej.

## Izolacja / budżet
`process` (osobny, minimalny), `always`. RAM ≤ 5 MB; heartbeat co 1 s; kill-switch (logika, 50 prób): p95 ≈ 1,2 ms; z zawieszonym Brokerem ≈ 102 ms.

## Konfiguracja (klucze TOML)
`[watchdog] heartbeat_timeout = "5s"`, `max_restarts = 3`, `window = "10m"`, `cooldown = "30m"`, `safe_mode_after_crash_loop = true`, `auto_rollback = true`; `kill_switch.hotkey` (współdzielony z `[security]`).

## Testy akceptacyjne
- `ACC-F3-watchdog-01`: kill-switch < 200 ms p95 z 50 prób — logika na atrapach (ściśle przy `ALFA_PERF_BUDGETS=1`); zawieszony Broker (1,5 s) nie blokuje: drzewa zabite, wynik w ~100 ms (`tests/daemon.rs`); prawdziwy system (hook, Job Objects, audio) — CI self-hosted.
- `ACC-F3-watchdog-02`: zabity sidecar → restart; 4. awaria w oknie → safe-mode, jądro i Broker-UI działają (`tests/watchdog.rs`).
- `ACC-F3-watchdog-03`: brak heartbeatu jądra → restart jądra (wirtualny zegar).

## Fake
`watchdog-fake`: rejestruje heartbeaty, awarie i kill-switch jako zdarzenia (bez zabijania), safe-mode z testu, akcje `tick` ze skryptu.

## Otwarte pytania
- Hook skrótu kill-switcha: w watchdogu (sesja użytkownika) — zrobione; usługa Brokera w sesji 0 nie ma hooka. Watchdog uruchamia aplikacja (część 3); uruchamianie przez launcher `alfa.exe` z jądrem w Job Object watchdoga (`-- <jądro>`) — decyzja przy integracji launchera.
- Własna ikona zasobnika watchdoga (`Shell_NotifyIconW`) i heartbeat jądra przez potok — SPEC v2 (limit rozmiaru `platform-windows-impl`).
- Katalog awarii chaosowych (≥ 20) — `evals/` w F8.
