//! Stan Brokera poza procesem dla UI i rejestru modułów: tryb (usługa / przenośny), stan łącza
//! IPC, okno zatwierdzeń, watchdog. Jedno źródło prawdy dla `ApprovalWindow`, zdrowia modułu
//! `safety-broker` i zdarzenia `BrokerStatus` (powłoka subskrybuje [`KernelStatus::subscribe`]).

use std::sync::{Mutex, MutexGuard};

use app_api::dto::{BrokerLinkState, BrokerMode, BrokerStatusView, LocalizedText};
use core_registry_contract::HealthStatus;
use tokio::sync::watch;

/// Stan łącza IPC z Brokerem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkState {
    /// Trwa łączenie (start, ponowienie) — do połączenia prośby o zgodę są odrzucane.
    Connecting,
    /// Połączono i przywitano (rola `Core`).
    Connected,
    /// Zerwane — bezpieczny stan; powód po polsku.
    Lost(String),
}

#[derive(Debug, Clone)]
struct Parts {
    mode: BrokerMode,
    link: LinkState,
    window: bool,
    watchdog: bool,
}

/// Stan Brokera poza procesem (współdzielony przez łącze, nadzór procesów i okno zatwierdzeń).
#[derive(Debug)]
pub struct KernelStatus {
    parts: Mutex<Parts>,
    tx: watch::Sender<BrokerStatusView>,
}

fn lost_text(reason: &str) -> LocalizedText {
    LocalizedText::new(
        format!(
            "Połączenie z Sędzią Cieni zerwane ({reason}). Bezpieczny stan: wszystko, co wymaga zgody, \
             jest odrzucane; Alfa ponawia połączenie."
        ),
        format!(
            "Connection to the Shadow Judge lost ({reason}). Safe state: everything that needs approval \
             is denied; Alfa keeps reconnecting."
        ),
    )
}

fn unavailable_text(reason: &str) -> LocalizedText {
    LocalizedText::new(
        format!(
            "Brak izolowanego Sędziego Cieni ({reason}). Bezpieczny stan: agentki nie wykonają niczego, \
             co wymaga zgody. Zainstaluj Alfę ponownie (binarki Jądra) albo usługę Sędziego Cieni."
        ),
        format!(
            "No isolated Shadow Judge ({reason}). Safe state: agents will not do anything that needs \
             approval. Reinstall Alfa (kernel binaries) or the Shadow Judge service."
        ),
    )
}

fn connected_text(mode: BrokerMode, window: bool) -> Option<LocalizedText> {
    match mode {
        // Audyt A-12: o oknie z wysoką integralnością tylko wtedy, gdy okno naprawdę działa.
        BrokerMode::Service if !window => Some(LocalizedText::new(
            "Połączono z usługą Sędziego Cieni (osobne konto Windows), ale okno zatwierdzeń nie działa — \
             prośby o zgodę są odrzucane.",
            "Connected to the Shadow Judge service (separate Windows account), but the approval window \
             is not running — approval requests are denied.",
        )),
        BrokerMode::Service => Some(LocalizedText::new(
            "Połączono z usługą Sędziego Cieni: osobne konto Windows, okno zatwierdzeń z wysoką \
             integralnością (UIPI).",
            "Connected to the Shadow Judge service: separate Windows account, approval window with \
             high integrity (UIPI).",
        )),
        BrokerMode::Portable => Some(LocalizedText::new(
            "Tryb przenośny: Sędzia Cieni działa jako proces Alfy na Twoim koncie — bez osobnego konta \
             Windows i bez ochrony UIPI okna zatwierdzeń (słabsza izolacja). Pełną izolację daje \
             usługa Sędziego Cieni.",
            "Portable mode: the Shadow Judge runs as an Alfa process under your account — no separate \
             Windows account and no UIPI protection of the approval window (weaker isolation). \
             The Shadow Judge service gives full isolation.",
        )),
        BrokerMode::InProcess | BrokerMode::Unavailable => None,
    }
}

fn render(p: &Parts) -> BrokerStatusView {
    let (state, detail) = match &p.link {
        LinkState::Connected => (BrokerLinkState::Connected, connected_text(p.mode, p.window)),
        LinkState::Connecting => (
            BrokerLinkState::Connecting,
            Some(LocalizedText::new(
                "Łączenie z Sędzią Cieni… do czasu połączenia prośby o zgodę są odrzucane.",
                "Connecting to the Shadow Judge… approval requests are denied until connected.",
            )),
        ),
        LinkState::Lost(reason) if p.mode == BrokerMode::Unavailable => {
            (BrokerLinkState::Lost, Some(unavailable_text(reason)))
        }
        LinkState::Lost(reason) => (BrokerLinkState::Lost, Some(lost_text(reason))),
    };
    BrokerStatusView {
        mode: p.mode,
        state,
        approval_window: p.window && state == BrokerLinkState::Connected,
        watchdog: p.watchdog,
        // Izolacja tylko przy działającym łączu z usługą (audyt A-12): sama konfiguracja
        // „usługa” przy zerwanym łączu niczego nie chroni.
        isolated: p.mode == BrokerMode::Service && state == BrokerLinkState::Connected,
        detail,
    }
}

impl KernelStatus {
    /// Stan początkowy: łączenie; `window` — czy Broker uruchamia okno zatwierdzeń.
    pub fn new(mode: BrokerMode, window: bool) -> Self {
        let parts = Parts {
            mode,
            link: LinkState::Connecting,
            window,
            watchdog: false,
        };
        let (tx, _) = watch::channel(render(&parts));
        Self {
            parts: Mutex::new(parts),
            tx,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Parts> {
        self.parts.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn update(&self, change: impl FnOnce(&mut Parts)) {
        let view = {
            let mut parts = self.lock();
            change(&mut parts);
            render(&parts)
        };
        self.tx.send_if_modified(|current| {
            let changed = *current != view;
            if changed {
                *current = view;
            }
            changed
        });
    }

    /// Nowy stan łącza.
    pub fn set_link(&self, link: LinkState) {
        self.update(|p| p.link = link);
    }

    /// Watchdog działa (obsługuje `Ctrl+Shift+F12`) albo się zakończył.
    pub fn set_watchdog(&self, running: bool) {
        self.update(|p| p.watchdog = running);
    }

    /// Stan łącza.
    pub fn link(&self) -> LinkState {
        self.lock().link.clone()
    }

    /// Czy łącze działa.
    pub fn connected(&self) -> bool {
        self.lock().link == LinkState::Connected
    }

    /// Czy watchdog obsługuje kill-switch.
    pub fn watchdog(&self) -> bool {
        self.lock().watchdog
    }

    /// Widok dla UI.
    pub fn view(&self) -> BrokerStatusView {
        render(&self.lock())
    }

    /// Zmiany widoku (zdarzenie `BrokerStatus`).
    pub fn subscribe(&self) -> watch::Receiver<BrokerStatusView> {
        self.tx.subscribe()
    }

    /// Zdrowie modułu `safety-broker` w rejestrze.
    pub fn health(&self) -> HealthStatus {
        match self.link() {
            LinkState::Connected => HealthStatus::Healthy,
            LinkState::Connecting => HealthStatus::Degraded("łączenie z Sędzią Cieni".into()),
            LinkState::Lost(reason) => {
                HealthStatus::Unhealthy(format!("połączenie z Sędzią Cieni zerwane: {reason}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_is_marked_weaker_and_lost_is_safe_state() {
        let s = KernelStatus::new(BrokerMode::Portable, true);
        let mut rx = s.subscribe();
        assert_eq!(s.view().state, BrokerLinkState::Connecting);
        assert!(!s.view().approval_window, "okno tylko po połączeniu");
        s.set_link(LinkState::Connected);
        let v = s.view();
        assert!(v.approval_window && !v.isolated);
        assert!(v.detail.unwrap().pl.contains("słabsza izolacja"));
        assert!(rx.has_changed().unwrap());
        rx.mark_unchanged();
        s.set_link(LinkState::Connected);
        assert!(!rx.has_changed().unwrap(), "bez zmiany — bez zdarzenia");
        s.set_link(LinkState::Lost("rura".into()));
        let v = s.view();
        assert_eq!(v.state, BrokerLinkState::Lost);
        assert!(!v.approval_window);
        assert!(v.detail.unwrap().pl.contains("odrzucane"));
        assert!(matches!(s.health(), HealthStatus::Unhealthy(_)));
        s.set_watchdog(true);
        assert!(s.view().watchdog && s.watchdog());
    }

    #[test]
    fn service_is_isolated() {
        let s = KernelStatus::new(BrokerMode::Service, true);
        assert!(matches!(s.health(), HealthStatus::Degraded(_)));
        s.set_link(LinkState::Connected);
        assert!(s.view().isolated && s.connected());
        assert_eq!(s.health(), HealthStatus::Healthy);
    }

    /// Audyt A-12: „izolowany” tylko przy działającym łączu z usługą, a opis okna zatwierdzeń
    /// zgodny z tym, czy okno naprawdę działa.
    #[test]
    fn service_is_not_isolated_without_link_and_tells_about_missing_window() {
        let s = KernelStatus::new(BrokerMode::Service, true);
        assert!(!s.view().isolated, "łączenie — jeszcze bez izolacji");
        s.set_link(LinkState::Lost("rura zamknięta".into()));
        assert!(!s.view().isolated, "zerwane łącze — bez izolacji");
        let no_window = KernelStatus::new(BrokerMode::Service, false);
        no_window.set_link(LinkState::Connected);
        let view = no_window.view();
        assert!(view.isolated && !view.approval_window);
        let detail = view.detail.map(|d| d.pl).unwrap_or_default();
        assert!(detail.contains("okno zatwierdzeń nie działa"), "{detail}");
        assert!(!detail.contains("wysoką"), "{detail}");
    }
}
