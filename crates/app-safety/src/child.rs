//! Uruchamianie procesu pomocniczego „jak wywołujący” (tryb deweloperski `--console`): ten sam
//! token i poziom integralności co Broker — **bez ochrony UIPI** (głośno logowane przez usługę).
//! Dane startowe przez stdin, stan po uchwycie `Child` (bez wyścigu PID).

use std::collections::BTreeMap;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};

use platform_contract::{LaunchIntegrity, PlatformError, SessionLaunch, SessionLauncherPort};

/// Uruchamianie przez `std::process` (każdy system).
#[derive(Debug, Default)]
pub struct ChildLauncher {
    running: Mutex<BTreeMap<u32, Child>>,
}

impl ChildLauncher {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<u32, Child>> {
        self.running.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl SessionLauncherPort for ChildLauncher {
    fn launch(&self, spec: &SessionLaunch) -> Result<u32, PlatformError> {
        if !spec.image.is_absolute() {
            return Err(PlatformError::InvalidPath(spec.image.clone()));
        }
        if spec.integrity != LaunchIntegrity::AsCaller {
            return Err(PlatformError::Unsupported(
                "ChildLauncher: tylko as_caller (tryb deweloperski)".into(),
            ));
        }
        let io = |e: std::io::Error| PlatformError::Io(format!("{}: {e}", spec.image.display()));
        let mut child = Command::new(&spec.image)
            .args(&spec.args)
            .stdin(Stdio::piped())
            .spawn()
            .map_err(io)?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&spec.stdin).map_err(io)?;
        }
        let pid = child.id();
        self.lock().insert(pid, child);
        Ok(pid)
    }

    fn is_running(&self, pid: u32) -> bool {
        let mut map = self.lock();
        let alive = map
            .get_mut(&pid)
            .is_some_and(|c| matches!(c.try_wait(), Ok(None)));
        if !alive {
            map.remove(&pid);
        }
        alive
    }
}
