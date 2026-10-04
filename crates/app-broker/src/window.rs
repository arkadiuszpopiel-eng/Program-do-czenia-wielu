//! Okno zatwierdzeń przy Brokerze poza procesem: Broker-UI uruchamia i nadzoruje sam Broker
//! (usługa — w sesji użytkownika z wysoką integralnością; tryb przenośny — proces potomny
//! `alfa-broker --console`). Okno pokazuje wszystkie oczekujące prośby samo (najstarsza na
//! wierzchu, miganie zamiast kradzieży fokusu), więc aplikacja niczego mu nie wysyła — karta
//! w wątku tylko przenosi uwagę właściciela. **Zatwierdzanie nigdy w WebView** (PLAN §8.2).

use std::sync::Arc;

use app_api::dto::BrokerStatusView;
use app_api::error::AppError;
use app_api::ports::ApprovalWindow;

use crate::status::{KernelStatus, LinkState};

/// Okno Brokera i stan łącza dla UI.
#[derive(Debug, Clone)]
pub struct RemoteWindow {
    status: Arc<KernelStatus>,
}

impl RemoteWindow {
    /// Okno nad wspólnym stanem.
    pub fn new(status: Arc<KernelStatus>) -> Self {
        Self { status }
    }
}

impl ApprovalWindow for RemoteWindow {
    fn present(&self, _approval: &str) -> Result<(), AppError> {
        let view = self.status.view();
        if view.approval_window {
            return Ok(());
        }
        let why = match self.status.link() {
            LinkState::Lost(why) => format!("połączenie z Brokerem zerwane ({why})"),
            LinkState::Connecting => "trwa łączenie z Brokerem".to_owned(),
            LinkState::Connected => "okno zatwierdzeń Brokera nie jest uruchomione".to_owned(),
        };
        Err(AppError::forbidden(format!(
            "Ta prośba wymaga potwierdzenia w oknie Brokera, a {why} — bezpieczny stan: prośba \
             zostanie odrzucona."
        )))
    }

    fn available(&self) -> bool {
        self.status.view().approval_window
    }

    fn status(&self) -> Option<BrokerStatusView> {
        Some(self.status.view())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_api::dto::BrokerMode;

    #[test]
    fn present_only_when_connected_with_window() {
        let status = Arc::new(KernelStatus::new(BrokerMode::Portable, true));
        let w = RemoteWindow::new(status.clone());
        assert!(!w.available());
        assert!(w.present("1").unwrap_err().message.contains("łączenie"));
        status.set_link(LinkState::Connected);
        assert!(w.available() && w.present("1").is_ok());
        status.set_link(LinkState::Lost("x".into()));
        assert!(w.present("1").unwrap_err().message.contains("zerwane"));
        let no_ui = KernelStatus::new(BrokerMode::Portable, false);
        no_ui.set_link(LinkState::Connected);
        let w = RemoteWindow::new(Arc::new(no_ui));
        assert!(
            w.present("1")
                .unwrap_err()
                .message
                .contains("nie jest uruchomione")
        );
        assert_eq!(w.status().map(|s| s.mode), Some(BrokerMode::Portable));
    }
}
