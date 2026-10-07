# watchdog-contract

Kontrakt watchdoga (docs/modules/watchdog/SPEC.md, PLAN §8.6, §12.2): `Heartbeat`/`Health`, `WatchPolicy`
(timeout, `max_restarts` w oknie, cooldown rollbacku, safe-mode, auto-rollback), `WatchAction`, porty
`Supervisor`, `ConfigHistory` (`core-config`), `UpdaterSignal` (`updater`), trait `Watchdog`. Wspólne
z Brokerem: `KillSwitch`, `KillReason`, `KillReport`, `JobRegistry` + `JobTable` (zabijanie drzew przez
`ProcessPort::kill_tree`, błąd jednego nie zatrzymuje pozostałych), zdarzenie `kernel.audio.silence`,
zegar `Clock`/`ManualClock`/`SystemClock`. Współdzielone testy kontraktowe (`contract-tests`).
