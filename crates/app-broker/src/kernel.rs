//! Procesy Jądra przy starcie aplikacji (ADR 0003, PLAN §8.1–8.2, §8.6): wybór trybu
//! ([`crate::mode`]), w trybie przenośnym — `alfa-broker --console` jako proces potomny,
//! zawsze (gdy jest obok aplikacji) — `alfa-watchdog` z kill-switchem `Ctrl+Shift+F12` poza UI,
//! łącze IPC z nadzorem w osobnym wątku. Powłoka bierze stąd [`RemoteKernel`] do `AppOptions`,
//! subskrybuje stan (zdarzenie `BrokerStatus`) i kill-switch watchdoga („STOP WSZYSTKIEGO”).

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
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
    watchdog: Arc<Mutex<Option<Watchdog>>>,
    supervisor: Option<JoinHandle<()>>,
}

/// Co potrzeba do (ponownego) uruchomienia watchdoga z wątku nadzoru.
struct WatchdogStarter {
    exe_dir: PathBuf,
    spawner: Arc<dyn Spawner>,
    /// Argumenty bez `--broker-pid` (potok Brokera).
    base_args: Vec<String>,
    status: Arc<KernelStatus>,
    kills: Arc<watch::Sender<u64>>,
}

impl WatchdogStarter {
    /// Argumenty dla Brokera o numerze `pid` (tryb przenośny) albo bez numeru (usługa).
    fn args(&self, pid: Option<u32>) -> Vec<String> {
        let mut args = self.base_args.clone();
        if let Some(pid) = pid {
            args.extend(["--broker-pid".into(), pid.to_string()]);
        }
        args
    }
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
        let (kills, _) = watch::channel(0u64);
        let kills = Arc::new(kills);
        let starter = WatchdogStarter {
            exe_dir: setup.exe_dir.clone(),
            spawner: setup.spawner.clone(),
            base_args: plan.watchdog_args,
            status: status.clone(),
            kills: kills.clone(),
        };
        let mut watched = sup.broker_pid();
        let watchdog = Arc::new(Mutex::new(start_watchdog(&starter, starter.args(watched))));
        // Czekamy równolegle na pierwsze połączenie z Brokerem (tylko błędy przejściowe — odmowa
        // serwera albo uszkodzona konfiguracja usługi kończą czekanie od razu) i na gotowość
        // watchdoga, żeby aplikacja nie zajęła Ctrl+Shift+F12 przed nim. Wcześniej oba czekania szły
        // jedno po drugim, a 2 s na watchdoga kończyły się fałszywym błędem przy wolniejszym starcie.
        let dog_deadline = Instant::now() + setup.timing.watchdog_ready;
        loop {
            let now = Instant::now();
            let broker = status.link() == LinkState::Connecting && now < deadline;
            let dog = !status.watchdog() && now < dog_deadline && running(&watchdog);
            if !broker && !dog {
                break;
            }
            // Nadzór Brokera kroczy także w czasie czekania na watchdoga (awaria tuż po połączeniu).
            let wait = sup.step(true).min(if broker {
                Duration::from_millis(100)
            } else {
                Duration::from_millis(10)
            });
            std::thread::sleep(wait.max(Duration::from_millis(10)));
        }
        if status.link() == LinkState::Connecting {
            status.set_link(LinkState::Lost(format!(
                "Broker nie odpowiedział w ciągu {} s od startu",
                setup.timing.first_connect.as_secs()
            )));
        }
        if !status.watchdog() && running(&watchdog) {
            // Aplikacja NIE zajmuje skrótu: watchdog rejestruje go raz i przy zajętym kończy działanie
            // (przegląd 2026-10-22). Skrót przejmuje aplikacja dopiero, gdy watchdog naprawdę padnie.
            tracing::warn!(
                limit_s = setup.timing.watchdog_ready.as_secs(),
                "alfa-watchdog jeszcze nie gotowy — skrót Ctrl+Shift+F12 zostaje dla niego"
            );
            status.set_watchdog(true);
        }
        let stop = StopSignal::new();
        let thread_stop = stop.clone();
        let thread_watchdog = watchdog.clone();
        let supervisor = std::thread::Builder::new()
            .name("alfa-broker-supervisor".into())
            .spawn(move || {
                loop {
                    let wait = sup.step(false);
                    // Audyt A-05: Broker uruchomiony ponownie ma nowy PID — watchdog dostaje nową
                    // tożsamość (inaczej odrzuca potok nowego procesu i kill-switch do niego nie
                    // dociera).
                    let now = sup.broker_pid();
                    if now.is_some() && now != watched {
                        watched = now;
                        restart_watchdog(&starter, &thread_watchdog, now);
                    }
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
            watchdog,
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
            w.stop();
        }
        if let Some(t) = self.supervisor.take() {
            let _ = t.join();
        }
    }
}

/// Kończy watchdoga z poprzednią tożsamością Brokera i uruchamia nowego.
fn restart_watchdog(starter: &WatchdogStarter, slot: &Mutex<Option<Watchdog>>, pid: Option<u32>) {
    let mut slot = slot.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(mut old) = slot.take() {
        old.stop();
    }
    tracing::warn!(
        ?pid,
        "Broker uruchomiony ponownie — watchdog z nową tożsamością Brokera"
    );
    *slot = start_watchdog(starter, starter.args(pid));
}

/// Uruchamia `alfa-watchdog` (jeśli jest obok aplikacji); gotowość (skrót zarejestrowany) przychodzi
/// komunikatem na stdout (`notice::attach`) — bez blokowania wątku (start, nadzór Brokera).
fn start_watchdog(starter: &WatchdogStarter, mut args: Vec<String>) -> Option<Watchdog> {
    let image = mode::watchdog_image(&starter.exe_dir)?;
    args.push("--lifeline".into());
    let started = Instant::now();
    let mut child = match starter.spawner.spawn(&ChildSpec { image, args }) {
        Ok(child) => child,
        Err(e) => {
            tracing::error!(error = %e, "alfa-watchdog nie wystartował — skrót obsłuży aplikacja");
            return None;
        }
    };
    let replaced = Arc::new(AtomicBool::new(false));
    notice::attach(
        child.as_mut(),
        &starter.status,
        &starter.kills,
        started,
        replaced.clone(),
    );
    Some(Watchdog { child, replaced })
}

/// Proces watchdoga i znacznik celowego zakończenia (restart Brokera, zamknięcie aplikacji).
struct Watchdog {
    child: Box<dyn ChildProc>,
    replaced: Arc<AtomicBool>,
}

impl Watchdog {
    /// Celowe zakończenie: koniec strumienia nie oddaje skrótu aplikacji i nie jest błędem.
    fn stop(&mut self) {
        self.replaced
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.child.kill();
    }
}

/// Czy proces watchdoga działa.
fn running(slot: &Mutex<Option<Watchdog>>) -> bool {
    slot.lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_mut()
        .is_some_and(|w| w.child.running())
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
