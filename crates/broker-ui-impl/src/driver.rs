//! Pętla Broker-UI: synchronizacja kolejki z Brokerem (nowe prośby → karty, rozstrzygnięte
//! gdzie indziej / wygasłe / po kill-switchu → wycofane), decyzje → `Resolve`.

use std::collections::BTreeSet;
use std::time::Duration;

use broker_ui_contract::{BrokerLink, BrokerUi, UiConfig, UiError, UiEvent};
use platform_contract::StopSignal;
use safety_broker_contract::{ApprovalDecision, ApprovalId};
use watchdog_contract::Clock;

/// Wynik kroku pętli.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CycleReport {
    /// Rozstrzygnięta prośba (przekazana do Brokera).
    pub resolved: Option<ApprovalId>,
    /// Zdarzenia Broker-UI z tego kroku.
    pub events: Vec<UiEvent>,
}

/// Jeden krok: opcjonalnie synchronizacja z Brokerem, potem obsługa wejścia do `wait_ms`.
///
/// Odmowa, której Broker nie przyjął (np. dowód bez Windows Hello przy `hello_required`), i tak
/// kończy prośbę jako odrzuconą po stronie Brokera — błąd łącza zwracany jest tylko dla zgód.
pub fn cycle(
    ui: &mut dyn BrokerUi,
    link: &mut dyn BrokerLink,
    now_ms: u64,
    wait_ms: u32,
    sync: bool,
) -> Result<CycleReport, UiError> {
    if sync {
        let pending = link.pending()?;
        let ids: BTreeSet<ApprovalId> = pending.iter().map(|c| c.request.id).collect();
        for id in ui.queued().into_iter().filter(|id| !ids.contains(id)) {
            ui.withdraw(id);
        }
        for ch in pending {
            // Wyzwanie wygasłe w międzyczasie jest pomijane (Broker je przeterminuje).
            let _ = ui.show(ch, now_ms);
        }
    }
    let mut report = CycleReport::default();
    if let Some(decision) = ui.poll_decision(now_ms, wait_ms) {
        let id = decision.id;
        let deny = decision.decision == ApprovalDecision::Deny;
        match link.resolve(decision) {
            Ok(()) => report.resolved = Some(id),
            Err(_) if deny => report.resolved = Some(id),
            Err(e) => {
                report.events = ui.drain_events();
                return Err(e);
            }
        }
    }
    report.events = ui.drain_events();
    Ok(report)
}

/// Pętla do zatrzymania: synchronizacja co `config.poll_ms`, zdarzenia przekazywane do `sink`.
/// Błąd łącza kończy pętlę (proces zostanie uruchomiony ponownie przez usługę Brokera).
pub fn run(
    ui: &mut dyn BrokerUi,
    link: &mut dyn BrokerLink,
    clock: &dyn Clock,
    config: &UiConfig,
    stop: &StopSignal,
    sink: &mut dyn FnMut(&UiEvent),
) -> Result<(), UiError> {
    let mut last_sync: Option<u64> = None;
    let wait = u32::try_from(config.poll_ms.clamp(10, 1_000)).unwrap_or(250);
    while !stop.wait(Duration::ZERO) {
        let now = clock.now_ms();
        let sync = last_sync.is_none_or(|t| now.saturating_sub(t) >= config.poll_ms);
        if sync {
            last_sync = Some(now);
        }
        let report = cycle(ui, link, now, wait, sync)?;
        for e in &report.events {
            sink(e);
        }
    }
    Ok(())
}
