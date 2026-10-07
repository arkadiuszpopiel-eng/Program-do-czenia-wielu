//! Atrapy portów procesów Jądra: okno zatwierdzeń ze skryptem zdarzeń, uruchamianie w sesji
//! użytkownika, katalogi prywatne, host usługi, MMCSS i wolne miejsce na dysku.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use platform_contract::{
    ApprovalSurfacePort, DiskPort, DiskSpace, MmcssPort, MmcssTask, PlatformError, PrivateDirPort,
    ServiceBody, ServiceHostPort, SessionLaunch, SessionLauncherPort, Sid, StopSignal,
    SurfaceEvent, SurfaceView, ThreadBoost,
};

fn guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

#[derive(Debug, Default)]
struct SurfaceState {
    presented: Vec<SurfaceView>,
    visible: bool,
    events: VecDeque<SurfaceEvent>,
    dismissals: u32,
}

/// Okno zatwierdzeń w pamięci: test dopisuje zdarzenia ([`FakeSurface::push`]), sprawdza widoki.
#[derive(Debug, Default)]
pub struct FakeSurface {
    state: Mutex<SurfaceState>,
    ready: Condvar,
}

impl FakeSurface {
    /// Nowe, ukryte okno.
    pub fn new() -> Self {
        Self::default()
    }

    /// Dopisuje zdarzenie (np. kliknięcie z próbką wejścia).
    pub fn push(&self, event: SurfaceEvent) {
        guard(&self.state).events.push_back(event);
        self.ready.notify_all();
    }

    /// Wszystkie pokazane widoki (kolejno).
    pub fn presented(&self) -> Vec<SurfaceView> {
        guard(&self.state).presented.clone()
    }

    /// Ostatni widok, jeśli okno jest widoczne.
    pub fn current(&self) -> Option<SurfaceView> {
        let st = guard(&self.state);
        st.visible.then(|| st.presented.last().cloned()).flatten()
    }

    /// Liczba ukryć okna.
    pub fn dismissals(&self) -> u32 {
        guard(&self.state).dismissals
    }
}

impl ApprovalSurfacePort for FakeSurface {
    fn present(&self, view: &SurfaceView) -> Result<(), PlatformError> {
        view.validate()?;
        let mut st = guard(&self.state);
        st.presented.push(view.clone());
        st.visible = true;
        Ok(())
    }

    fn dismiss(&self) -> Result<(), PlatformError> {
        let mut st = guard(&self.state);
        if st.visible {
            st.dismissals += 1;
        }
        st.visible = false;
        Ok(())
    }

    fn next_event(&self, timeout_ms: u32) -> Option<SurfaceEvent> {
        let st = guard(&self.state);
        let (mut st, _) = self
            .ready
            .wait_timeout_while(st, Duration::from_millis(timeout_ms.into()), |s| {
                s.events.is_empty()
            })
            .unwrap_or_else(|p| p.into_inner());
        st.events.pop_front()
    }
}

#[derive(Debug, Default)]
struct LaunchState {
    launched: Vec<(u32, SessionLaunch)>,
    running: BTreeSet<u32>,
    next_pid: u32,
    fail: Option<String>,
}

/// Uruchamianie w sesji użytkownika: rejestruje specyfikacje, PID-y od 5000, test kończy procesy.
#[derive(Debug, Default)]
pub struct FakeLauncher {
    state: Mutex<LaunchState>,
}

impl FakeLauncher {
    /// Nowa atrapa.
    pub fn new() -> Self {
        Self::default()
    }

    /// Uruchomione (PID, specyfikacja).
    pub fn launched(&self) -> Vec<(u32, SessionLaunch)> {
        guard(&self.state).launched.clone()
    }

    /// Symuluje zakończenie procesu.
    pub fn exit(&self, pid: u32) {
        guard(&self.state).running.remove(&pid);
    }

    /// Kolejne uruchomienia kończą się błędem (`None` = znowu działają).
    pub fn fail_with(&self, reason: Option<&str>) {
        guard(&self.state).fail = reason.map(str::to_owned);
    }
}

impl SessionLauncherPort for FakeLauncher {
    fn launch(&self, spec: &SessionLaunch) -> Result<u32, PlatformError> {
        let mut st = guard(&self.state);
        if let Some(reason) = &st.fail {
            return Err(PlatformError::PermissionDenied(reason.clone()));
        }
        st.next_pid += 1;
        let pid = 5000 + st.next_pid;
        st.launched.push((pid, spec.clone()));
        st.running.insert(pid);
        Ok(pid)
    }

    fn is_running(&self, pid: u32) -> bool {
        guard(&self.state).running.contains(&pid)
    }
}

/// Katalogi prywatne: zapamiętuje (ścieżka, właściciel); katalog tworzy naprawdę (testy plikowe).
#[derive(Debug, Default)]
pub struct FakePrivateDirs {
    ensured: Mutex<Vec<(PathBuf, Sid)>>,
}

impl FakePrivateDirs {
    /// Wywołania `ensure_private_dir`.
    pub fn ensured(&self) -> Vec<(PathBuf, Sid)> {
        guard(&self.ensured).clone()
    }
}

impl PrivateDirPort for FakePrivateDirs {
    fn ensure_private_dir(&self, path: &Path, owner: &Sid) -> Result<(), PlatformError> {
        std::fs::create_dir_all(path).map_err(|e| PlatformError::Io(e.to_string()))?;
        guard(&self.ensured).push((path.to_path_buf(), owner.clone()));
        Ok(())
    }
}

/// Host usługi: wykonuje ciało na bieżącym wątku; test zatrzymuje je przez [`Self::stop_signal`].
#[derive(Debug, Default)]
pub struct FakeServiceHost {
    signal: StopSignal,
    names: Mutex<Vec<String>>,
}

impl FakeServiceHost {
    /// Sygnał przekazywany ciału usługi.
    pub fn stop_signal(&self) -> StopSignal {
        self.signal.clone()
    }

    /// Nazwy uruchomionych usług.
    pub fn names(&self) -> Vec<String> {
        guard(&self.names).clone()
    }
}

impl ServiceHostPort for FakeServiceHost {
    fn run_service(&self, name: &str, body: ServiceBody) -> Result<(), PlatformError> {
        guard(&self.names).push(name.to_owned());
        body(self.signal.clone()).map_err(PlatformError::Io)
    }
}

/// MMCSS: liczy podniesienia i zwroty priorytetu.
#[derive(Debug, Default)]
pub struct FakeMmcss {
    boosts: AtomicUsize,
    reverts: Arc<AtomicUsize>,
}

impl FakeMmcss {
    /// (podniesienia, zwroty).
    pub fn counts(&self) -> (usize, usize) {
        (
            self.boosts.load(Ordering::SeqCst),
            self.reverts.load(Ordering::SeqCst),
        )
    }
}

impl MmcssPort for FakeMmcss {
    fn boost_current_thread(&self, task: MmcssTask) -> Result<ThreadBoost, PlatformError> {
        self.boosts.fetch_add(1, Ordering::SeqCst);
        let reverts = self.reverts.clone();
        Ok(ThreadBoost::new(task, move || {
            reverts.fetch_add(1, Ordering::SeqCst);
        }))
    }
}

/// Dysk: miejsce ustawiane per prefiks ścieżki (najdłuższe dopasowanie), domyślnie 100 GB wolne.
#[derive(Debug, Default)]
pub struct FakeDisk {
    volumes: Mutex<BTreeMap<PathBuf, DiskSpace>>,
}

impl FakeDisk {
    /// Ustawia miejsce dla woluminu o korzeniu `root`.
    pub fn set(&self, root: impl Into<PathBuf>, space: DiskSpace) {
        guard(&self.volumes).insert(root.into(), space);
    }
}

impl DiskPort for FakeDisk {
    fn free_disk_space(&self, path: &Path) -> Result<DiskSpace, PlatformError> {
        if path.as_os_str().is_empty() {
            return Err(PlatformError::InvalidPath(path.to_path_buf()));
        }
        let volumes = guard(&self.volumes);
        let best = volumes
            .iter()
            .filter(|(root, _)| path.starts_with(root))
            .max_by_key(|(root, _)| root.as_os_str().len())
            .map(|(_, s)| *s);
        const GB: u64 = 1024 * 1024 * 1024;
        Ok(best.unwrap_or(DiskSpace {
            available_bytes: 100 * GB,
            total_bytes: 500 * GB,
            free_bytes: 100 * GB,
        }))
    }
}
