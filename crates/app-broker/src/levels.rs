//! Poziomy autonomii po stronie aplikacji (strażnik autonomii w `app-core`; wydzielone stąd ze
//! względu na limit 8000 linii crate'a): zapis obniżeń sesji w konfiguracji
//! (`identyfikator-sesji:poziom` — konfiguracja nie przyjmuje obiektów, stąd listy napisów),
//! podniesienia czekające na zgodę w oknie Brokera i odczyt stanu takiej prośby z Brokera.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use app_api::dto::AutonomyLevel;
use app_api::ports::ApprovalOutcome;
use safety_broker_contract::{ApprovalId, ApprovalStatus, Broker, BrokerError, Holder};

fn level_text(level: AutonomyLevel) -> Option<String> {
    serde_json::to_value(level)
        .ok()?
        .as_str()
        .map(str::to_owned)
}

fn level_of(text: &str) -> Option<AutonomyLevel> {
    serde_json::from_value(serde_json::Value::String(text.into())).ok()
}

/// Obniżenia sesji z wpisów `identyfikator-sesji:poziom` (wpisy nieczytelne są pomijane).
pub fn parse_session_levels(entries: Vec<String>) -> BTreeMap<String, AutonomyLevel> {
    entries
        .into_iter()
        .filter_map(|entry| {
            let (id, level) = entry.rsplit_once(':')?;
            Some((id.to_owned(), level_of(level)?))
        })
        .collect()
}

/// Wpisy `identyfikator-sesji:poziom`.
pub fn format_session_levels(levels: &BTreeMap<String, AutonomyLevel>) -> Vec<String> {
    levels
        .iter()
        .map(|(id, level)| format!("{id}:{}", level_text(*level).unwrap_or_default()))
        .collect()
}

/// Jak długo prośba o podniesienie czeka na stan z Brokera (karta Brokera wygasa po 10 min).
pub const RAISE_TTL: Duration = Duration::from_secs(15 * 60);

/// Podniesienie czekające na zgodę w oknie Brokera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRaise {
    /// Sesja; `None` — poziom globalny.
    pub session: Option<String>,
    /// Poziom, o który poproszono.
    pub level: AutonomyLevel,
    /// Identyfikator prośby w Brokerze (`request_id` z `request_level`).
    pub approval: String,
    at: Instant,
}

/// Podniesienia czekające na zgodę — tylko w pamięci procesu aplikacji, nie w konfiguracji, a do
/// przyjęcia potrzebna jest zgoda potwierdzona przez Brokera (przegląd 2026-10-21: sama zgodność
/// poziomu nie odróżniała zgody od nowego Brokera z poziomem domyślnym L3 — prośba odrzucona albo
/// bez decyzji i ponowne uruchomienie w ciągu 15 min nadpisywały obniżenie właściciela).
#[derive(Debug, Default)]
pub struct PendingRaises {
    items: Mutex<Vec<PendingRaise>>,
    /// Budzi strażnika przy nowej prośbie (śpi do 15 s, gdy nic nie czeka).
    wake: tokio::sync::Notify,
}

impl PendingRaises {
    fn lock(&self) -> MutexGuard<'_, Vec<PendingRaise>> {
        self.items.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Czeka na nową prośbę (prośba dodana, gdy nikt nie czekał, budzi od razu następne czekanie).
    pub async fn added(&self) {
        self.wake.notified().await;
    }

    /// Nowa prośba. Ten sam identyfikator zastępuje starszy wpis: Broker uruchomiony od nowa
    /// numeruje prośby od początku, więc stary wpis dotyczyłby cudzej prośby.
    pub fn add(&self, session: Option<&str>, level: AutonomyLevel, approval: &str) {
        self.add_at(session, level, approval, Instant::now());
    }

    fn add_at(&self, session: Option<&str>, level: AutonomyLevel, approval: &str, now: Instant) {
        let mut items = self.lock();
        items.retain(|r| r.approval != approval && now.duration_since(r.at) < RAISE_TTL);
        items.push(PendingRaise {
            session: session.map(str::to_owned),
            level,
            approval: approval.to_owned(),
            at: now,
        });
        drop(items);
        self.wake.notify_one();
    }

    /// Wpisy ważne teraz (przeterminowane są usuwane).
    pub fn current(&self) -> Vec<PendingRaise> {
        self.current_at(Instant::now())
    }

    fn current_at(&self, now: Instant) -> Vec<PendingRaise> {
        let mut items = self.lock();
        items.retain(|r| now.duration_since(r.at) < RAISE_TTL);
        items.clone()
    }

    /// Usuwa prośbę (rozstrzygniętą w Brokerze).
    pub fn remove(&self, approval: &str) {
        self.lock().retain(|r| r.approval != approval);
    }

    /// Czy coś czeka (strażnik sprawdza wtedy częściej).
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }
}

/// Podmiot prośby o zmianę poziomu: Broker zapisuje prośby z interfejsu jako prośby Jądra
/// (`system_holder` w `safety-broker-impl/src/apply.rs`), a stan pokazuje tylko proszącemu.
fn owner() -> Holder {
    Holder {
        session: core_bus_contract::SessionId::new("kernel"),
        agent: None,
        role: Some("owner".into()),
    }
}

/// Stan prośby o zmianę poziomu w Brokerze. Nieznana prośba (np. Broker uruchomiony od nowa)
/// i prośba innego podmiotu — `Rejected`; Broker nieosiągalny — `None` (bez decyzji).
pub fn approval_outcome(broker: &dyn Broker, approval: &str) -> Option<ApprovalOutcome> {
    let Ok(id) = approval.parse::<u64>() else {
        return Some(ApprovalOutcome::Rejected);
    };
    match broker.approval_status(ApprovalId(id), &owner()) {
        Ok(ApprovalStatus::Pending) => Some(ApprovalOutcome::Pending),
        Ok(ApprovalStatus::Approved { .. }) => Some(ApprovalOutcome::Approved),
        Ok(ApprovalStatus::Denied | ApprovalStatus::Expired)
        | Err(BrokerError::UnknownApproval(_) | BrokerError::Unauthorized(_)) => {
            Some(ApprovalOutcome::Rejected)
        }
        Err(e) => {
            tracing::debug!(error = %e, "stan prośby o zmianę poziomu nieznany");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_entries_round_trip_and_bad_ones_are_skipped() {
        let sessions: BTreeMap<String, AutonomyLevel> = [
            ("s-1".to_owned(), AutonomyLevel::L0),
            ("a:b".to_owned(), AutonomyLevel::L2),
        ]
        .into();
        let text = format_session_levels(&sessions);
        assert_eq!(text, vec!["a:b:L2".to_owned(), "s-1:L0".to_owned()]);
        assert_eq!(parse_session_levels(text), sessions);
        assert!(parse_session_levels(vec!["bez-poziomu".into(), "s:L9".into()]).is_empty());
    }

    #[test]
    fn pending_raises_expire_and_a_reused_id_replaces_the_old_entry() {
        let raises = PendingRaises::default();
        let t0 = Instant::now();
        raises.add_at(None, AutonomyLevel::L4, "1", t0);
        raises.add_at(Some("s"), AutonomyLevel::L3, "2", t0);
        // Druga prośba o ten sam zakres nie usuwa pierwszej (dwie karty w oknie Brokera).
        raises.add_at(None, AutonomyLevel::L2, "3", t0);
        assert_eq!(raises.current_at(t0).len(), 3);
        // Ten sam identyfikator od nowego Brokera — stary wpis znika.
        raises.add_at(Some("x"), AutonomyLevel::L1, "1", t0);
        let now = raises.current_at(t0);
        assert_eq!(now.len(), 3);
        assert!(
            now.iter()
                .any(|r| r.approval == "1" && r.session.as_deref() == Some("x"))
        );
        raises.remove("2");
        assert_eq!(raises.current_at(t0).len(), 2);
        assert!(!raises.is_empty());
        assert!(raises.current_at(t0 + RAISE_TTL).is_empty());
        assert!(raises.is_empty());
    }
}
