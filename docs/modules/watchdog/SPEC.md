# watchdog — SPEC (szkic v0)

## Cel
Osobny proces nadzoru: heartbeat jądra i modułów `process`, restart modułów, **safe-mode** po N awariach, rollback do ostatniej dobrej wersji/konfiguracji (z `updater`), obsługa kill-switch (skrót globalny, zasobnik) i zabijanie drzew procesów przez Job Objects — poza UI, < 200 ms (PLAN §8.6, §12.2, §3.1).

## Fala i priorytet
F3. P0. Diagnosta (klasteryzacja, propozycje) — F8, osobny moduł.

## Kontrakt (szkic Rust)
```rust
// watchdog-contract — SZKIC
pub struct Heartbeat { pub from: ProcessRole /* Core | Sidecar(ModuleId) | BrokerUi */, pub ts: Instant, pub health: HealthReport }
pub enum WatchAction { RestartModule(ModuleId), RestartCore, SafeMode { reason: String }, RollbackVersion, RollbackConfig(RevisionId), KillAll(KillReason) }
pub struct Policy { pub heartbeat_timeout: Duration, pub max_restarts: u8, pub window: Duration, pub cooldown: Duration }
pub trait Watchdog: Send + Sync {
    fn heartbeat(&self, hb: Heartbeat);
    fn register_job(&self, job: JobHandle, owner: ProcessRole);       // wszystkie drzewa procesów Alfy
    fn kill_switch(&self, reason: KillReason) -> Result<()>;           // < 200 ms: audio cisza + Job Objects
    fn actions(&self) -> Subscription<WatchAction>;
    fn safe_mode(&self) -> bool;
}
```
Zdarzenia (Diagnostics + Audyt dla kill/safe-mode): `watchdog.heartbeat.missed`, `watchdog.restart`, `watchdog.crash_loop`, `watchdog.safe_mode.entered/left`, `watchdog.rollback`, `watchdog.kill_switch { latency_ms }`.

## Zależności
`core-bus-contract` (przez pipe), `safety-broker-contract` (kill-switch współobsługiwany; Audyt), `updater-contract` (rollback wersji), `core-config-contract` (rollback konfiguracji), `platform-windows-contract` (Job Objects, hook skrótu, zasobnik). Minimalne zależności — musi działać, gdy jądro leży.

## Niezmienniki
- Watchdog nie zależy od jądra, UI ani modeli; startuje pierwszy (z launchera) i przeżywa awarię jądra.
- Kill-switch: skrót globalny (`Ctrl+Shift+F12`, reguła AltGr), przycisk w kapsule/zasobniku (przekazany bez WebView), „stop" głosem (przez `voice-cmd` → Broker) — od klawisza do ciszy audio i zabicia wszystkich Job Objects < 200 ms p95.
- Restarty ograniczone (`max_restarts` w `window`), potem safe-mode: tylko jądro + UI + Broker, bez głosu/narzędzi/agentek; wyjście z safe-mode ręczne.
- Rollback tylko do wersji/konfiguracji oznaczonej jako „ostatnia dobra" (health po starcie); nigdy w pętli (cooldown).
- Kill-switch nie wymaga zatwierdzenia; nigdy nie jest blokowany przez brak Brokera.
- Zabijanie = całe drzewa (Job Objects), także sidecary i mosty CLI.

## Zdolności / uprawnienia
Brak tokenów; działa jako Ty (kill własnych procesów) — bez elewacji.

## Izolacja
`process` (osobny, minimalny binarny), `always`.

## Budżet zasobów
RAM ≤ 5 MB; CPU ≈ 0 w bezczynności; heartbeat co 1 s (konfigurowalnie); reakcja na skrót ≤ 20 ms + zabicie ≤ 180 ms.

## Konfiguracja (klucze TOML)
`[watchdog] heartbeat_timeout = "5s"`, `max_restarts = 3`, `window = "10m"`, `cooldown = "30m"`, `safe_mode_after_crash_loop = true`, `auto_rollback = true`; `kill_switch.hotkey` (współdzielony z `[security]`).

## Wkład do UI
Stan „crash-loop modułu" i „safe-mode" (§14.4), Zdrowie systemu (F8), ostrzeżenie o rollbacku, „Co nowego" po rollbacku.

## Testy akceptacyjne
- `ACC-F3-watchdog-01`: kill-switch < 200 ms p95 z 50 prób pod obciążeniem UI (od klawisza do ciszy audio i zabicia Job Objects).
- `ACC-F3-watchdog-02`: chaos — zabity sidecar STT/LLM → restart ≤ 3 s; 4. awaria w oknie → safe-mode, jądro i UI działają.
- `ACC-F3-watchdog-03`: jądro zawieszone (brak heartbeat) → restart jądra, sesje nietknięte (dziennik append-only).

## Fake
`watchdog-fake`: rejestruje heartbeaty i akcje, kill-switch jako zdarzenie (bez zabijania) — testy `ui-quick`, `safety-broker`.

## Otwarte pytania
- Kto trzyma hook skrótu kill-switch: watchdog czy usługa Brokera (usługa w sesji 0 nie ma hooka klawiatury → watchdog w sesji użytkownika) — ADR (3)/(15), do ustalenia w SPEC v1.
- Katalog awarii chaosowych (≥ 20) — `evals/` w F8, część już w F3.
