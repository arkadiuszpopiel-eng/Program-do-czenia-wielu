//! `alfa-broker-ui`: bilet startowy ze stdin (jedna linia JSON od usługi Brokera) → łącze przez
//! named pipe (konto serwera = konto usługi z biletu) → natywne okno zatwierdzeń do zamknięcia
//! połączenia (wtedy proces kończy się, a usługa uruchamia go ponownie z nowym biletem).

use std::io::BufRead;
use std::sync::Arc;

use broker_ui_contract::{NoHello, UiConfig, UiEvent};
use broker_ui_impl::{NativeBrokerUi, PipeLink, driver};
use platform_contract::{ApprovalSurfacePort, ProcessIdentityPort, SecurePipePort, StopSignal};
use safety_broker_contract::ipc_blocking::UiLaunchTicket;
use watchdog_contract::Clock;

/// Limit długości biletu (bajty).
const MAX_TICKET: usize = 16 * 1024;

/// Czyta bilet startowy (pierwsza linia wejścia).
pub fn read_ticket<R: BufRead>(input: R) -> Result<UiLaunchTicket, String> {
    let mut line = String::new();
    let mut limited = input.take(u64::try_from(MAX_TICKET).unwrap_or(u64::MAX));
    limited
        .read_line(&mut line)
        .map_err(|e| format!("stdin: {e}"))?;
    if line.trim().is_empty() {
        return Err("brak biletu startowego na stdin (Broker-UI uruchamia usługa Brokera)".into());
    }
    serde_json::from_str(line.trim()).map_err(|e| format!("bilet startowy: {e}"))
}

/// Zapis zdarzenia Broker-UI do dziennika procesu (stderr).
pub fn log_event(e: &UiEvent) {
    eprintln!("[alfa-broker-ui] {}: {e:?}", e.name());
}

/// Pętla okna do zatrzymania albo zerwania połączenia z Brokerem.
pub fn run<S: ApprovalSurfacePort + 'static>(
    ticket: &UiLaunchTicket,
    pipes: &dyn SecurePipePort,
    identity: &dyn ProcessIdentityPort,
    surface: Arc<S>,
    clock: &dyn Clock,
    stop: &StopSignal,
) -> Result<(), String> {
    let mut link = PipeLink::connect(pipes, identity, ticket, std::process::id())
        .map_err(|e| e.to_string())?;
    let config = UiConfig::default();
    let mut ui = NativeBrokerUi::new(surface, Arc::new(NoHello), config);
    driver::run(&mut ui, &mut link, clock, &config, stop, &mut log_event).map_err(|e| e.to_string())
}
