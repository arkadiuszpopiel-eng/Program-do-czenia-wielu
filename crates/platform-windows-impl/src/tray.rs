//! `TrayPort` jako adapter: ikonę zasobnika i toasty rysuje powłoka Tauri 2 (`tray-icon`,
//! wtyczka powiadomień), a ten port przechowuje stan i menu oraz przekazuje je do podłączonego
//! backendu. Własna ikona `Shell_NotifyIconW` byłaby drugą ikoną tej samej aplikacji — świadomie
//! jej nie tworzymy (decyzja F1; zob. README).

use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use platform_contract::{Notification, PlatformError, TrayMenuItem, TrayPort, TrayState};

/// Backend zasobnika (implementuje go powłoka aplikacji, np. `shell-integration` na Tauri).
pub trait TrayBackend: Send + Sync {
    /// Zmienia ikonę/podpowiedź zgodnie ze stanem.
    fn apply_state(&self, state: TrayState) -> Result<(), PlatformError>;
    /// Podmienia menu.
    fn apply_menu(&self, items: &[TrayMenuItem]) -> Result<(), PlatformError>;
    /// Pokazuje powiadomienie.
    fn show_notification(&self, notification: &Notification) -> Result<(), PlatformError>;
}

/// Adapter zasobnika: stan + menu w pamięci, opcjonalny backend.
#[derive(Default)]
pub struct TrayAdapter {
    state: Mutex<(TrayState, Vec<TrayMenuItem>)>,
    backend: RwLock<Option<Arc<dyn TrayBackend>>>,
}

impl std::fmt::Debug for TrayAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrayAdapter")
            .field("state", &self.lock().0)
            .field("has_backend", &self.backend().is_some())
            .finish()
    }
}

impl TrayAdapter {
    fn lock(&self) -> MutexGuard<'_, (TrayState, Vec<TrayMenuItem>)> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn backend(&self) -> Option<Arc<dyn TrayBackend>> {
        self.backend
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Podłącza backend i od razu odtwarza w nim bieżący stan i menu.
    pub fn attach(&self, backend: Arc<dyn TrayBackend>) -> Result<(), PlatformError> {
        let (state, menu) = self.lock().clone();
        backend.apply_state(state)?;
        backend.apply_menu(&menu)?;
        *self.backend.write().unwrap_or_else(|p| p.into_inner()) = Some(backend);
        Ok(())
    }

    /// Odłącza backend (np. przy zamykaniu powłoki).
    pub fn detach(&self) {
        *self.backend.write().unwrap_or_else(|p| p.into_inner()) = None;
    }

    /// Bieżące menu.
    pub fn menu(&self) -> Vec<TrayMenuItem> {
        self.lock().1.clone()
    }
}

impl TrayPort for TrayAdapter {
    fn set_state(&self, state: TrayState) -> Result<(), PlatformError> {
        self.lock().0 = state;
        self.backend().map_or(Ok(()), |b| b.apply_state(state))
    }

    fn state(&self) -> TrayState {
        self.lock().0
    }

    fn set_menu(&self, items: Vec<TrayMenuItem>) -> Result<(), PlatformError> {
        let backend = self.backend();
        if let Some(b) = &backend {
            b.apply_menu(&items)?;
        }
        self.lock().1 = items;
        Ok(())
    }

    fn notify(&self, notification: Notification) -> Result<(), PlatformError> {
        match self.backend() {
            Some(b) => b.show_notification(&notification),
            None => Err(PlatformError::Unsupported(
                "brak backendu zasobnika — powłoka Tauri nie podłączyła adaptera".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder {
        log: Mutex<Vec<String>>,
    }

    impl TrayBackend for Recorder {
        fn apply_state(&self, state: TrayState) -> Result<(), PlatformError> {
            self.log.lock().unwrap().push(format!("state:{state:?}"));
            Ok(())
        }
        fn apply_menu(&self, items: &[TrayMenuItem]) -> Result<(), PlatformError> {
            self.log
                .lock()
                .unwrap()
                .push(format!("menu:{}", items.len()));
            Ok(())
        }
        fn show_notification(&self, n: &Notification) -> Result<(), PlatformError> {
            self.log.lock().unwrap().push(format!("toast:{}", n.title));
            Ok(())
        }
    }

    #[test]
    fn stores_state_and_replays_it_to_backend() {
        let tray = TrayAdapter::default();
        let toast = Notification {
            title: "Alfa".into(),
            body: "gotowe".into(),
        };
        assert!(matches!(
            tray.notify(toast.clone()),
            Err(PlatformError::Unsupported(_))
        ));
        tray.set_state(TrayState::Listening).unwrap();
        tray.set_menu(vec![TrayMenuItem {
            id: "quit".into(),
            label: "Zakończ".into(),
            enabled: true,
        }])
        .unwrap();
        let recorder = Arc::new(Recorder::default());
        tray.attach(recorder.clone()).unwrap();
        tray.set_state(TrayState::Working).unwrap();
        tray.notify(toast).unwrap();
        assert_eq!(tray.state(), TrayState::Working);
        assert_eq!(tray.menu().len(), 1);
        assert_eq!(
            *recorder.log.lock().unwrap(),
            vec!["state:Listening", "menu:1", "state:Working", "toast:Alfa"]
        );
        assert!(format!("{tray:?}").contains("has_backend: true"));
        tray.detach();
        assert!(
            tray.notify(Notification {
                title: "x".into(),
                body: String::new()
            })
            .is_err()
        );
    }
}
