//! Porty Jądra bezpieczeństwa (F3, część 2; docs/modules/platform-windows/SPEC.md): named pipe
//! z DACL na SID-y (`PIPE_REJECT_REMOTE_CLIENTS`, `FILE_FLAG_FIRST_PIPE_INSTANCE`, etykieta
//! integralności), tożsamość klienta potoku (obraz, SID tokenu, integralność, sesja), katalogi
//! prywatne, MMCSS „Pro Audio”, wolne miejsce, host usługi Windows, uruchamianie Broker-UI w sesji
//! użytkownika z wysoką integralnością, natywne okno zatwierdzeń z rozpoznawaniem wstrzyknięć.
//! Poza Windows wszystko zwraca `Unsupported` (logikę testuje się na `platform-fake`).

#[cfg(not(windows))]
mod portable;
mod surface;
#[cfg(windows)]
mod win_launch;
#[cfg(windows)]
mod win_pipe;
#[cfg(windows)]
mod win_sec;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use platform_contract::{
    DiskPort, DiskSpace, LaunchIntegrity, MmcssPort, MmcssTask, PeerIdentity, PipeConnection,
    PipeListener, PipeSecurity, PlatformError, PrivateDirPort, ProcessIdentityPort, SecurePipePort,
    ServiceBody, ServiceHostPort, SessionLaunch, SessionLauncherPort, Sid, ThreadBoost,
};

#[cfg(not(windows))]
use portable as os;
#[cfg(not(windows))]
use portable as launch;
#[cfg(not(windows))]
use portable as pipe;
#[cfg(windows)]
use win_launch as launch;
#[cfg(windows)]
use win_pipe as pipe;
#[cfg(windows)]
use win_sec as os;

pub use surface::WinApprovalSurface;

/// Bezstanowe porty Jądra Windows: potoki, tożsamość procesów, katalogi prywatne, MMCSS, dysk,
/// host usługi.
#[derive(Debug, Clone, Copy, Default)]
pub struct WinKernel;

impl SecurePipePort for WinKernel {
    fn listen(&self, security: &PipeSecurity) -> Result<Box<dyn PipeListener>, PlatformError> {
        pipe::listen(security)
    }

    fn connect(
        &self,
        name: &str,
        timeout_ms: u32,
    ) -> Result<Box<dyn PipeConnection>, PlatformError> {
        pipe::connect(name, timeout_ms)
    }
}

impl ProcessIdentityPort for WinKernel {
    fn identify(&self, pid: u32) -> Result<PeerIdentity, PlatformError> {
        os::identify(pid)
    }

    fn current_user(&self) -> Result<Sid, PlatformError> {
        os::current_user()
    }
}

impl PrivateDirPort for WinKernel {
    fn ensure_private_dir(&self, path: &Path, owner: &Sid) -> Result<(), PlatformError> {
        os::ensure_private_dir(path, owner)
    }
}

impl MmcssPort for WinKernel {
    fn boost_current_thread(&self, task: MmcssTask) -> Result<ThreadBoost, PlatformError> {
        os::mmcss_boost(task)
    }
}

impl DiskPort for WinKernel {
    fn free_disk_space(&self, path: &Path) -> Result<DiskSpace, PlatformError> {
        os::free_disk_space(path)
    }
}

impl ServiceHostPort for WinKernel {
    fn run_service(&self, name: &str, body: ServiceBody) -> Result<(), PlatformError> {
        launch::run_service(name, body)
    }
}

/// Uruchamianie Broker-UI w sesji konsoli z wysoką integralnością (`UserSessionHigh`; usługa
/// z `SeTcbPrivilege`). Bilet startowy przez stdin. Tryb „jak wywołujący” (deweloperski) jest
/// przenośny i żyje w korzeniu kompozycji (`app-safety`).
#[derive(Debug, Default)]
pub struct WinSessionLauncher {
    running: Mutex<BTreeMap<u32, launch::Proc>>,
}

impl WinSessionLauncher {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<u32, launch::Proc>> {
        self.running.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl SessionLauncherPort for WinSessionLauncher {
    fn launch(&self, spec: &SessionLaunch) -> Result<u32, PlatformError> {
        if !spec.image.is_absolute() {
            return Err(PlatformError::InvalidPath(spec.image.clone()));
        }
        if spec.integrity != LaunchIntegrity::UserSessionHigh {
            return Err(PlatformError::Unsupported(
                "WinSessionLauncher: tylko user_session_high".into(),
            ));
        }
        let (pid, proc) = launch::launch_high(spec)?;
        self.lock().insert(pid, proc);
        Ok(pid)
    }

    fn is_running(&self, pid: u32) -> bool {
        let mut map = self.lock();
        let alive = map.get(&pid).is_some_and(launch::proc_running);
        if !alive {
            map.remove(&pid);
        }
        alive
    }
}
