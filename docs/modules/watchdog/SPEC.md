# watchdog — SPEC (v1: logika zaimplementowana — część 1; proces i hook skrótu — część 2)

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
- `ACC-F3-watchdog-01`: kill-switch < 200 ms p95 z 50 prób — logika na atrapach (ściśle przy `ALFA_PERF_BUDGETS=1`); prawdziwy system (hook, Job Objects, audio) — część 2, CI self-hosted.
- `ACC-F3-watchdog-02`: zabity sidecar → restart; 4. awaria w oknie → safe-mode, jądro i Broker-UI działają (`tests/watchdog.rs`).
- `ACC-F3-watchdog-03`: brak heartbeatu jądra → restart jądra (wirtualny zegar).

## Fake
`watchdog-fake`: rejestruje heartbeaty, awarie i kill-switch jako zdarzenia (bez zabijania), safe-mode z testu, akcje `tick` ze skryptu.

## Otwarte pytania
- Hook skrótu kill-switcha w watchdogu (sesja użytkownika) — usługa Brokera w sesji 0 nie ma hooka; ADR (3)/(15) w części 2.
- Katalog awarii chaosowych (≥ 20) — `evals/` w F8.
