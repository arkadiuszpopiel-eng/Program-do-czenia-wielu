//! `alfa-watchdog`: skrót `Ctrl+Shift+F12` (hook + `RegisterHotKey` z `platform-windows-impl`)
//! → kill-switch: cisza audio → zabicie zarejestrowanych drzew procesów (Job Objects; jądro
//! uruchomione przez watchdoga z `--` jest w jego Job Object razem z potomkami) → Broker przez
//! IPC (`KillAll`, rola `Watchdog`, zapis po tożsamości obrazu) z limitem 100 ms.
//!
//! Uruchomiony przez aplikację (`app-broker`): `--lifeline` — kończy się, gdy aplikacja zamknie
//! stdin; komunikaty dla aplikacji jako linie JSON na stdout ([`notice_ready`], [`notice_kill`]):
//! aplikacja nie rejestruje skrótu drugi raz i po kill-switchu zatrzymuje generacje i narzędzia.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use platform_contract::{
    Integrity, ProcessIdentityPort, ProcessPort, ProcessSpec, SecurePipePort, Sid, StopSignal,
};
use platform_windows_impl::{JobLimits, WinHotkeys, WinProcesses};
use platform_windows_kernel_impl::WinKernel;
use safety_broker_contract::ipc::{
    ClientCredential, ClientRole, Hello, PROTOCOL_VERSION, Request, Response,
};
use safety_broker_contract::ipc_blocking::BlockingClient;
use watchdog_contract::{
    Clock, JobRegistry, KillReason, KillReport, ProcessRole, Supervisor, SystemClock,
    UpdaterSignal, WatchPolicy,
};
use watchdog_impl::daemon::{KillSwitchDaemon, ThreadedPeer};
use watchdog_impl::{WatchdogPorts, WatchdogService};

/// Ile czekamy na wolną instancję potoku Brokera (ms) — reszta limitu 100 ms na odpowiedź.
const CONNECT_MS: u32 = 50;

/// Kill-switch Brokera przez potok: powitanie roli `Watchdog` bez MAC (Broker przyjmuje je
/// tylko od obrazu `alfa-watchdog.exe` na koncie użytkownika), konto serwera sprawdzane.
pub fn broker_peer(
    pipes: Arc<dyn SecurePipePort>,
    identity: Arc<dyn ProcessIdentityPort>,
    pipe: String,
    broker_user: Option<Sid>,
) -> ThreadedPeer {
    broker_peer_checked(pipes, identity, pipe, broker_user, None)
}

/// Jak [`broker_peer`], a dodatkowo serwer potoku musi być procesem `broker_pid` (tryb
/// przenośny: Broker to proces potomny aplikacji na tym samym koncie — inny proces tego konta
/// mógłby utworzyć kolejną instancję potoku).
pub fn broker_peer_checked(
    pipes: Arc<dyn SecurePipePort>,
    identity: Arc<dyn ProcessIdentityPort>,
    pipe: String,
    broker_user: Option<Sid>,
    broker_pid: Option<u32>,
) -> ThreadedPeer {
    ThreadedPeer::new(move |reason: KillReason| -> Result<KillReport, String> {
        let conn = pipes
            .connect(&pipe, CONNECT_MS)
            .map_err(|e| e.to_string())?;
        if let Some(pid) = broker_pid.filter(|p| *p != conn.peer_pid()) {
            return Err(format!(
                "serwer potoku to proces {}, a nie Broker {pid} — podstawiony?",
                conn.peer_pid()
            ));
        }
        if let Some(expected) = &broker_user {
            let server = identity
                .identify(conn.peer_pid())
                .map_err(|e| e.to_string())?;
            if server.user != *expected {
                return Err(format!(
                    "serwer potoku na koncie {} — podstawiony?",
                    server.user
                ));
            }
        }
        let credential = ClientCredential {
            client_id: "watchdog".into(),
            role: ClientRole::Watchdog,
            expires_at_ms: 0,
            mac: String::new(),
        };
        let hello = Hello {
            protocol: PROTOCOL_VERSION,
            credential,
            pid: std::process::id(),
            sid: None,
            image: None,
        };
        let mut client = BlockingClient::connect(conn, &hello).map_err(|e| e.to_string())?;
        match client.call(Request::KillAll { reason }) {
            Ok(Response::Killed(report)) => Ok(report),
            Ok(other) => Err(format!("nieoczekiwana odpowiedź Brokera: {other:?}")),
            Err(e) => Err(e.to_string()),
        }
    })
}

/// Nadzorca bez restartów (F3: watchdog obsługuje kill-switch; heartbeat i restart — gdy
/// procesy zaczną wysyłać heartbeat przez IPC).
#[derive(Debug, Default)]
pub struct NoRestart;

impl Supervisor for NoRestart {
    fn restart(&self, role: &ProcessRole) -> Result<(), String> {
        Err(format!("restart {role} nieobsługiwany w tym trybie"))
    }

    fn stop(&self, role: &ProcessRole) -> Result<(), String> {
        Err(format!("zatrzymanie {role} nieobsługiwane w tym trybie"))
    }
}

/// Argumenty: `--broker-pipe NAZWA`, `--broker-user SID`, `--broker-pid PID`, `--lifeline`,
/// `-- <jądro> [argumenty…]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchdogArgs {
    /// Potok Brokera.
    pub broker_pipe: String,
    /// Konto usługi Brokera (sprawdzane po stronie klienta).
    pub broker_user: Option<Sid>,
    /// PID procesu Brokera (tryb przenośny — sprawdzany po stronie klienta).
    pub broker_pid: Option<u32>,
    /// Zakończ, gdy proces nadrzędny (aplikacja) zamknie stdin.
    pub lifeline: bool,
    /// Polecenie jądra uruchamianego w Job Object watchdoga.
    pub core: Option<(PathBuf, Vec<String>)>,
}

impl WatchdogArgs {
    /// Parsuje argumenty (bez nazwy programu).
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let (own, core) = match args.iter().position(|a| a == "--") {
            Some(i) => (&args[..i], args.get(i + 1..)),
            None => (args, None),
        };
        let broker_user = crate::arg_value(own, "--broker-user")
            .map(|s| Sid::parse(&s).map_err(|e| e.to_string()))
            .transpose()?;
        let core = core
            .and_then(|c| c.split_first())
            .map(|(cmd, rest)| (PathBuf::from(cmd), rest.to_vec()));
        let broker_pipe = crate::arg_value(own, "--broker-pipe")
            .unwrap_or_else(|| crate::broker::DEV_PIPE.to_owned());
        let broker_pid = crate::arg_value(own, "--broker-pid")
            .map(|p| p.parse::<u32>().map_err(|e| format!("--broker-pid: {e}")))
            .transpose()?;
        Ok(Self {
            broker_pipe,
            broker_user,
            broker_pid,
            lifeline: crate::has_flag(own, "--lifeline"),
            core,
        })
    }
}

/// Sygnał rollbacku dla launchera (`updater-impl`): katalog instalacji z obrazu jądra
/// (`<root>\versions\<ver>\alfa-desktop.exe`), inaczej `%LOCALAPPDATA%\Alfa`; wersja
/// uruchomiona = wersja tego pakietu. Błąd odczytu instalacji — bez sygnału (watchdog działa dalej).
pub fn updater_signal(core: Option<&std::path::Path>) -> Option<Arc<dyn UpdaterSignal>> {
    let local = std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Alfa"))?;
    updater_signal_in(core, &local)
}

/// Jak [`updater_signal`], z jawnym katalogiem danych Alfy (testy, tryb przenośny).
pub fn updater_signal_in(
    core: Option<&std::path::Path>,
    local: &std::path::Path,
) -> Option<Arc<dyn UpdaterSignal>> {
    let root = updater_impl::launcher::app_install_root(core, local);
    let updater = updater_impl::FsUpdater::new(updater_impl::UpdaterConfig::new(&root)).ok()?;
    let running = semver::Version::parse(env!("CARGO_PKG_VERSION")).ok()?;
    let signal = updater_impl::WatchdogSignal::new(Arc::new(updater), running);
    Some(Arc::new(signal))
}

/// Uruchamia watchdoga na Windows (blokuje).
pub fn run(args: WatchdogArgs) -> Result<(), String> {
    let processes = Arc::new(WinProcesses::new(JobLimits::default()));
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let kernel = Arc::new(WinKernel);
    let peer = broker_peer_checked(
        kernel.clone(),
        kernel,
        args.broker_pipe,
        args.broker_user,
        args.broker_pid,
    );
    let stop = StopSignal::new();
    if args.lifeline {
        crate::spawn_lifeline(stop.clone());
    }
    let ports = WatchdogPorts {
        clock: clock.clone(),
        processes: processes.clone(),
        supervisor: Arc::new(NoRestart),
        config: None,
        updater: updater_signal(args.core.as_ref().map(|(cmd, _)| cmd.as_path())),
        bus: None,
        audit: None,
        peers: vec![Arc::new(peer)],
    };
    let service = Arc::new(WatchdogService::new(WatchPolicy::default(), ports));
    if let Some((cmd, args)) = args.core {
        let cwd = cmd.parent().map(PathBuf::from).unwrap_or_default();
        let spec = ProcessSpec {
            cmd,
            args,
            cwd,
            integrity: Integrity::Medium,
            memory_limit_mb: None,
        };
        let handle = processes.spawn(spec).map_err(|e| e.to_string())?;
        service.register_job(handle, ProcessRole::Core, "jądro Alfy");
    }
    let hotkeys = WinHotkeys::new();
    let id = hotkeys.register_kill_switch().map_err(|e| e.to_string())?;
    let mut daemon = KillSwitchDaemon::new(service, clock, id);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .map_err(|e| e.to_string())?;
    eprintln!("[alfa-watchdog] kill-switch Ctrl+Shift+F12 aktywny");
    notify(&notice_ready());
    while !stop.is_stopped() {
        let events = hotkeys.wait_events(Duration::from_secs(1));
        if let Some(report) = runtime.block_on(daemon.on_events(&events)) {
            eprintln!("[alfa-watchdog] kill-switch: {report:?}");
            notify(&notice_kill(&report));
        }
    }
    Ok(())
}

/// Komunikat „skrót zarejestrowany” (aplikacja nie rejestruje `Ctrl+Shift+F12` drugi raz).
pub fn notice_ready() -> String {
    serde_json::json!({ "event": "ready", "hotkey": "Ctrl+Shift+F12" }).to_string()
}

/// Komunikat „kill-switch wykonany” (aplikacja zatrzymuje generacje, przebiegi i narzędzia).
pub fn notice_kill(report: &KillReport) -> String {
    serde_json::json!({
        "event": "kill_switch",
        "reason": report.reason,
        "tokens_revoked": report.tokens_revoked,
        "jobs_killed": report.jobs_killed,
        "latency_us": report.latency_us,
    })
    .to_string()
}

/// Jedna linia komunikatu na stdout (odczytuje ją wyłącznie aplikacja — anonimowy potok).
fn notify(line: &str) {
    let mut out = std::io::stdout().lock();
    if writeln!(out, "{line}").and_then(|()| out.flush()).is_err() {
        eprintln!("[alfa-watchdog] stdout zamknięty — aplikacja nie dostanie komunikatu");
    }
}
