//! `alfa-watchdog`: skrót `Ctrl+Shift+F12` (hook + `RegisterHotKey` z `platform-windows-impl`)
//! → kill-switch: cisza audio → zabicie zarejestrowanych drzew procesów (Job Objects; jądro
//! uruchomione przez watchdoga z `--` jest w jego Job Object razem z potomkami) → Broker przez
//! IPC (`KillAll`, rola `Watchdog`, zapis po tożsamości obrazu) z limitem 100 ms.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use platform_contract::{
    Integrity, ProcessIdentityPort, ProcessPort, ProcessSpec, SecurePipePort, Sid,
};
use platform_windows_impl::{JobLimits, WinHotkeys, WinKernel, WinProcesses};
use safety_broker_contract::ipc::{
    ClientCredential, ClientRole, Hello, PROTOCOL_VERSION, Request, Response,
};
use safety_broker_contract::ipc_blocking::BlockingClient;
use watchdog_contract::{
    Clock, JobRegistry, KillReason, KillReport, ProcessRole, Supervisor, SystemClock, WatchPolicy,
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
    ThreadedPeer::new(move |reason: KillReason| -> Result<KillReport, String> {
        let conn = pipes
            .connect(&pipe, CONNECT_MS)
            .map_err(|e| e.to_string())?;
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

/// Argumenty: `--broker-pipe NAZWA`, `--broker-user SID`, `-- <jądro> [argumenty…]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchdogArgs {
    /// Potok Brokera.
    pub broker_pipe: String,
    /// Konto usługi Brokera (sprawdzane po stronie klienta).
    pub broker_user: Option<Sid>,
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
        Ok(Self {
            broker_pipe,
            broker_user,
            core,
        })
    }
}

/// Uruchamia watchdoga na Windows (blokuje).
pub fn run(args: WatchdogArgs) -> Result<(), String> {
    let processes = Arc::new(WinProcesses::new(JobLimits::default()));
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let kernel = Arc::new(WinKernel);
    let peer = broker_peer(kernel.clone(), kernel, args.broker_pipe, args.broker_user);
    let ports = WatchdogPorts {
        clock: clock.clone(),
        processes: processes.clone(),
        supervisor: Arc::new(NoRestart),
        config: None,
        updater: None,
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
    loop {
        let events = hotkeys.wait_events(Duration::from_secs(1));
        if let Some(report) = runtime.block_on(daemon.on_events(&events)) {
            eprintln!("[alfa-watchdog] kill-switch: {report:?}");
        }
    }
}
