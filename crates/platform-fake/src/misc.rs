//! Fake okien, procesów i zasobnika.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use platform_contract::{
    Notification, PlatformError, ProcessHandle, ProcessPort, ProcessSpec, ProcessStatus,
    TrayMenuItem, TrayPort, TrayState, WindowId, WindowInfo, WindowPort,
};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Lista okien w pamięci.
#[derive(Debug, Default)]
pub struct FakeWindows {
    windows: Mutex<Vec<WindowInfo>>,
}

impl FakeWindows {
    /// Dodaje okno do listy.
    pub fn add(&self, window: WindowInfo) {
        lock(&self.windows).push(window);
    }

    /// Ustawia flagę pełnego ekranu okna.
    pub fn set_fullscreen(&self, id: WindowId, fullscreen: bool) {
        for w in lock(&self.windows).iter_mut().filter(|w| w.id == id) {
            w.fullscreen = fullscreen;
        }
    }
}

impl WindowPort for FakeWindows {
    fn list(&self) -> Vec<WindowInfo> {
        lock(&self.windows).clone()
    }

    fn focus(&self, id: WindowId) -> Result<(), PlatformError> {
        let mut ws = lock(&self.windows);
        if !ws.iter().any(|w| w.id == id) {
            return Err(PlatformError::UnknownResource(format!("okno {}", id.0)));
        }
        for w in ws.iter_mut() {
            w.focused = w.id == id;
        }
        Ok(())
    }
}

/// Procesy w pamięci — nic nie jest uruchamiane.
#[derive(Debug, Default)]
pub struct FakeProcesses {
    procs: Mutex<BTreeMap<u32, (ProcessSpec, ProcessStatus)>>,
    next: Mutex<u32>,
    elevated: Mutex<bool>,
}

impl FakeProcesses {
    /// Specyfikacje uruchomionych procesów (do asercji).
    pub fn spawned(&self) -> Vec<(ProcessHandle, ProcessSpec)> {
        lock(&self.procs)
            .iter()
            .map(|(h, (spec, _))| (ProcessHandle(*h), spec.clone()))
            .collect()
    }

    /// Symuluje zakończenie procesu z kodem.
    pub fn set_exited(&self, handle: ProcessHandle, code: i32) -> Result<(), PlatformError> {
        lock(&self.procs)
            .get_mut(&handle.0)
            .map(|(_, status)| *status = ProcessStatus::Exited(code))
            .ok_or_else(|| PlatformError::UnknownResource(format!("proces {}", handle.0)))
    }

    /// Symuluje okno administratora na pierwszym planie.
    pub fn set_foreground_elevated(&self, elevated: bool) {
        *lock(&self.elevated) = elevated;
    }
}

impl ProcessPort for FakeProcesses {
    fn spawn(&self, spec: ProcessSpec) -> Result<ProcessHandle, PlatformError> {
        if spec.cmd.as_os_str().is_empty() {
            return Err(PlatformError::InvalidPath(spec.cmd));
        }
        let mut next = lock(&self.next);
        *next += 1;
        let handle = ProcessHandle(*next);
        lock(&self.procs).insert(handle.0, (spec, ProcessStatus::Running));
        Ok(handle)
    }

    fn kill_tree(&self, handle: ProcessHandle) -> Result<(), PlatformError> {
        lock(&self.procs)
            .get_mut(&handle.0)
            .map(|(_, status)| *status = ProcessStatus::Killed)
            .ok_or_else(|| PlatformError::UnknownResource(format!("proces {}", handle.0)))
    }

    fn status(&self, handle: ProcessHandle) -> Result<ProcessStatus, PlatformError> {
        lock(&self.procs)
            .get(&handle.0)
            .map(|(_, status)| *status)
            .ok_or_else(|| PlatformError::UnknownResource(format!("proces {}", handle.0)))
    }

    fn foreground_is_elevated(&self) -> bool {
        *lock(&self.elevated)
    }
}

/// Zasobnik w pamięci: stan, menu i lista pokazanych powiadomień.
#[derive(Debug, Default)]
pub struct FakeTray {
    state: Mutex<TrayState>,
    menu: Mutex<Vec<TrayMenuItem>>,
    notifications: Mutex<Vec<Notification>>,
}

impl FakeTray {
    /// Pokazane powiadomienia.
    pub fn notifications(&self) -> Vec<Notification> {
        lock(&self.notifications).clone()
    }

    /// Bieżące menu.
    pub fn menu(&self) -> Vec<TrayMenuItem> {
        lock(&self.menu).clone()
    }
}

impl TrayPort for FakeTray {
    fn set_state(&self, state: TrayState) -> Result<(), PlatformError> {
        *lock(&self.state) = state;
        Ok(())
    }

    fn state(&self) -> TrayState {
        *lock(&self.state)
    }

    fn set_menu(&self, items: Vec<TrayMenuItem>) -> Result<(), PlatformError> {
        *lock(&self.menu) = items;
        Ok(())
    }

    fn notify(&self, notification: Notification) -> Result<(), PlatformError> {
        lock(&self.notifications).push(notification);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn windows_focus_and_fullscreen() {
        let w = FakeWindows::default();
        let info = |id: u64| WindowInfo {
            id: WindowId(id),
            title: format!("w{id}"),
            process: "app".into(),
            focused: false,
            fullscreen: false,
        };
        w.add(info(1));
        w.add(info(2));
        assert!(w.focus(WindowId(9)).is_err());
        w.focus(WindowId(2)).unwrap();
        assert!(!w.fullscreen_app_active());
        w.set_fullscreen(WindowId(2), true);
        assert!(w.fullscreen_app_active());
    }

    #[test]
    fn processes_lifecycle() {
        let p = FakeProcesses::default();
        let spec = ProcessSpec {
            cmd: PathBuf::from("/bin/whisper"),
            args: vec![],
            cwd: PathBuf::from("/"),
            integrity: Default::default(),
            memory_limit_mb: Some(512),
        };
        let h = p.spawn(spec.clone()).unwrap();
        assert_eq!(p.status(h).unwrap(), ProcessStatus::Running);
        p.set_exited(h, 3).unwrap();
        assert_eq!(p.status(h).unwrap(), ProcessStatus::Exited(3));
        p.kill_tree(h).unwrap();
        assert_eq!(p.status(h).unwrap(), ProcessStatus::Killed);
        assert_eq!(p.spawned().len(), 1);
        assert!(p
            .spawn(ProcessSpec {
                cmd: PathBuf::new(),
                ..spec
            })
            .is_err());
        assert!(!p.foreground_is_elevated());
        p.set_foreground_elevated(true);
        assert!(p.foreground_is_elevated());
    }

    #[test]
    fn tray_records() {
        let t = FakeTray::default();
        t.set_state(TrayState::Working).unwrap();
        t.set_menu(vec![TrayMenuItem {
            id: "quit".into(),
            label: "Zakończ".into(),
            enabled: true,
        }])
        .unwrap();
        t.notify(Notification {
            title: "a".into(),
            body: "b".into(),
        })
        .unwrap();
        assert_eq!(t.state(), TrayState::Working);
        assert_eq!(t.menu().len(), 1);
        assert_eq!(t.notifications().len(), 1);
    }
}
