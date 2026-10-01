# watchdog-impl

Watchdog — logika (`WatchdogService`): heartbeat z timeoutem (`tick` na wirtualnym zegarze), restart
z limitem w oknie, po pętli awarii safe-mode (tylko jądro, Broker, Broker-UI i procesy krytyczne;
wyjście ręczne), rollback konfiguracji i wersji do „ostatniej dobrej” z cooldownem (nigdy w pętli),
kill-switch: cisza audio → zabicie drzew Job Objects → peer Brokera (unieważnienie tokenów, limit 100 ms)
→ Audyt (limit 50 ms) — brak lub zawieszenie Brokera nie blokuje kill-switcha. Zdarzenia z metod
synchronicznych w kolejce `flush_events` (safe-mode/rollback także do Audytu). Binarka procesu, hook
`Ctrl+Shift+F12` i zasobnik — część 2.

**Część 2** (`daemon`): `KillSwitchDaemon` (zdarzenia skrótu `Ctrl+Shift+F12` → `kill_all(Hotkey)`, odbicie 300 ms),
`ThreadedPeer` (Broker wołany blokująco przez IPC na osobnym wątku — limit 100 ms działa przy zawieszonym Brokerze).
Binarka `alfa-watchdog` — crate `app-safety`. Testy: `tests/daemon.rs`.
