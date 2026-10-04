//! Procesy Jądra przy starcie aplikacji (ADR 0003, PLAN §8.1–8.2, §8.6): wybór trybu
//! ([`crate::mode`]), w trybie przenośnym — `alfa-broker --console` jako proces potomny,
//! zawsze (gdy jest obok aplikacji) — `alfa-watchdog` z kill-switchem `Ctrl+Shift+F12` poza UI,
//! łącze IPC z nadzorem w osobnym wątku. Powłoka bierze stąd [`RemoteKernel`] do `AppOptions`,
//! subskrybuje stan (zdarzenie `BrokerStatus`) i kill-switch watchdoga („STOP WSZYSTKIEGO”).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use app_api::dto::{BrokerMode, BrokerStatusView};
use platform_contract::{ProcessIdentityPort, SecurePipePort, StopSignal};
use tokio::sync::watch;

use crate::children::{ChildProc, ChildSpec, Spawner, StdSpawner};
use crate::link::{BrokerLink, LinkConfig, ServerCheck};
use crate::mode::{self, Detected, PORTABLE_PIPE};
use crate::status::{KernelStatus, LinkState};
use crate::supervise::{Supervisor, Target, Timing};
use crate::{RemoteKernel, notice};

/// Wejście startu.
pub struct KernelSetup {
    /// Katalog aplikacji (wersji) z binarkami Jądra.
    pub exe_dir: PathBuf,
    /// `broker.json` usługi (`None` — nie sprawdzać usługi).
    pub service_config: Option<PathBuf>,
    /// Potoki z ACL.
    pub pipes: Arc<dyn SecurePipePort>,
    /// Tożsamość procesów (serwer potoku usługi).
    pub identity: Arc<dyn ProcessIdentityPort>,
    /// Uruchamianie procesów potomnych.
    pub spawner: Arc<dyn Spawner>,
    /// Odstępy nadzoru.
    pub timing: Timing,
}

impl KernelSetup {
    /// Porty Windows (`platform-windows-kernel-impl`) i `std::process`; katalog aplikacji —
    /// katalog bieżącego pliku wykonywalnego, konfiguracja usługi — `%ProgramData%`.
    pub fn system() -> Result<Self, String> {
        let exe = std::env::current_exe().map_err(|e| format!("ścieżka aplikacji: {e}"))?;
        let exe_dir = exe
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| "brak katalogu aplikacji".to_owned())?;
        let service_config =
            std::env::var_os("ProgramData").map(|p| mode::service_config_path(&PathBuf::from(p)));
        let kernel = Arc::new(platform_windows_kernel_impl::WinKernel);
        Ok(Self {
            exe_dir,
            service_config,
            pipes: kernel.clone(),
            identity: kernel,
            spawner: Arc::new(StdSpawner),
            timing: Timing::default(),
        })
    }
}

/// Wynik startu.
pub enum KernelStart {
    /// Broker poza procesem (usługa albo tryb przenośny).
    Remote(KernelProcesses),
    /// Broker w procesie (powód po polsku).
    InProcess(String),
}

/// Procesy Jądra uruchomione przez aplikację i łącze z Brokerem; zamknięcie (drop) zatrzymuje
/// nadzór i kończy procesy potomne.
pub struct KernelProcesses {
    status: Arc<KernelStatus>,
    link: Arc<BrokerLink>,
    kills: Arc<watch::Sender<u64>>,
    stop: StopSignal,
    watchdog: Mutex<Option<Box<dyn ChildProc>>>,
    supervisor: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for KernelProcesses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KernelProcesses")
            .field("status", &self.status.view())
            .finish_non_exhaustive()
    }
}

struct Plan {
    mode: BrokerMode,
    window: bool,
    pipe: String,
    target: Target,
    watchdog_args: Vec<String>,
}

fn plan(setup: &KernelSetup) -> Result<Plan, String> {
    match mode::detect(&setup.exe_dir, setup.service_config.as_deref()) {
        Detected::InProcess(why) => Err(why),
        Detected::Service(Ok(t)) => Ok(Plan {
            mode: BrokerMode::Service,
            window: t.window,
            watchdog_args: vec![
                "--broker-pipe".into(),
                t.pipe.clone(),
                "--broker-user".into(),
                t.broker_user.to_string(),
            ],
            pipe: t.pipe,
            target: Target::Service(ServerCheck::Service {
                user: Some(t.broker_user),
            }),
        }),
        Detected::Service(Err(why)) => Ok(Plan {
            mode: BrokerMode::Service,
            window: false,
            pipe: "alfa-broker".into(),
            target: Target::Broken(why),
            watchdog_args: vec!["--broker-pipe".into(), "alfa-broker".into()],
        }),
        Detected::Portable { broker, window } => Ok(Plan {
            mode: BrokerMode::Portable,
            window,
            pipe: PORTABLE_PIPE.into(),
            target: Target::Portable {
                spec: ChildSpec {
                    image: broker,
                    args: vec!["--console".into(), "--lifeline".into()],
                },
                spawner: setup.spawner.clone(),
            },
            watchdog_args: vec!["--broker-pipe".into(), PORTABLE_PIPE.into()],
        }),
    }
}

impl KernelProcesses {
    /// Wykrywa tryb i uruchamia procesy Jądra; czeka (co najwyżej `timing.first_connect`) na
    /// pierwsze połączenie, żeby aplikacja wystartowała z działającym Brokerem.
    pub fn start(setup: KernelSetup) -> KernelStart {
        let plan = match plan(&setup) {
            Ok(plan) => plan,
            Err(why) => return KernelStart::InProcess(why),
        };
        let status = Arc::new(KernelStatus::new(plan.mode, plan.window));
        let link = BrokerLink::new(
            setup.pipes.clone(),
            setup.identity.clone(),
            LinkConfig::new(plan.pipe.clone()),
            status.clone(),
        );
        let mut sup = Supervisor::new(link.clone(), plan.target, setup.timing);
        let deadline = Instant::now() + setup.timing.first_connect;
        let _ = sup.step(true);
        let mut watchdog_args = plan.watchdog_args;
        if let Some(pid) = sup.broker_pid() {
            watchdog_args.extend(["--broker-pid".into(), pid.to_string()]);
        }
        let (kills, _) = watch::channel(0u64);
        let kills = Arc::new(kills);
        let watchdog = start_watchdog(&setup, watchdog_args, &status, &kills);
        // Czekamy tylko na błędy przejściowe (potok jeszcze nie istnieje); odmowa serwera albo
        // uszkodzona konfiguracja usługi kończą czekanie od razu (stan „zerwane” z powodem).
        while status.link() == LinkState::Connecting && Instant::now() < deadline {
            let wait = sup.step(true).min(Duration::from_millis(100));
            std::thread::sleep(wait.max(Duration::from_millis(10)));
        }
        if status.link() == LinkState::Connecting {
            status.set_link(LinkState::Lost(format!(
                "Broker nie odpowiedział w ciągu {} s od startu",
                setup.timing.first_connect.as_secs()
            )));
        }
        let stop = StopSignal::new();
        let thread_stop = stop.clone();
        let supervisor = std::thread::Builder::new()
            .name("alfa-broker-supervisor".into())
            .spawn(move || {
                loop {
                    let wait = sup.step(false);
                    if thread_stop.wait(wait) {
                        break;
                    }
                }
            })
            .map_err(|e| tracing::error!(error = %e, "nadzór Brokera nie wystartował"))
            .ok();
        KernelStart::Remote(Self {
            status,
            link,
            kills,
            stop,
            watchdog: Mutex::new(watchdog),
            supervisor,
        })
    }

    /// Broker dla `AppOptions::kernel`.
    pub fn remote(&self) -> RemoteKernel {
        RemoteKernel::new(self.link.clone())
    }

    /// Stan dla UI (zdarzenie `BrokerStatus` przy każdej zmianie).
    pub fn subscribe(&self) -> watch::Receiver<BrokerStatusView> {
        self.status.subscribe()
    }

    /// Bieżący stan.
    pub fn view(&self) -> BrokerStatusView {
        self.status.view()
    }

    /// Kill-switch wykonany przez watchdoga (licznik rośnie przy każdym naciśnięciu).
    pub fn kills(&self) -> watch::Receiver<u64> {
        self.kills.subscribe()
    }

    /// Czy `Ctrl+Shift+F12` obsługuje watchdog (aplikacja nie rejestruje skrótu drugi raz).
    pub fn watchdog_active(&self) -> bool {
        self.status.watchdog()
    }
}

impl Drop for KernelProcesses {
    fn drop(&mut self) {
        self.stop.stop();
        self.link.disconnect("zamknięcie aplikacji");
        if let Some(mut w) = self
            .watchdog
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            w.kill();
        }
        if let Some(t) = self.supervisor.take() {
            let _ = t.join();
        }
    }
}

/// Uruchamia `alfa-watchdog` (jeśli jest obok aplikacji) i czeka na gotowość (skrót zarejestrowany).
fn start_watchdog(
    setup: &KernelSetup,
    mut args: Vec<String>,
    status: &Arc<KernelStatus>,
    kills: &Arc<watch::Sender<u64>>,
) -> Option<Box<dyn ChildProc>> {
    let image = mode::watchdog_image(&setup.exe_dir)?;
    args.push("--lifeline".into());
    let mut child = match setup.spawner.spawn(&ChildSpec { image, args }) {
        Ok(child) => child,
        Err(e) => {
            tracing::error!(error = %e, "alfa-watchdog nie wystartował — skrót obsłuży aplikacja");
            return None;
        }
    };
    let last = notice::attach(child.as_mut(), status, kills);
    let deadline = Instant::now() + setup.timing.watchdog_ready;
    while !status.watchdog() && child.running() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if !status.watchdog() {
        tracing::error!(
            ostatnia_linia = ?last.get(),
            "alfa-watchdog nie zgłosił gotowości — skrót Ctrl+Shift+F12 obsłuży aplikacja"
        );
    }
    Some(child)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn handles_can_live_in_shell_state() {
        // Powłoka Tauri trzyma je w stanie zarządzanym i przekazuje do zadań asynchronicznych.
        send_sync::<KernelProcesses>();
        send_sync::<RemoteKernel>();
        send_sync::<crate::KernelBroker>();
    }
}
