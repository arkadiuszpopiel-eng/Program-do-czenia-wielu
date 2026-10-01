//! `alfa-broker-ui` — okno zatwierdzeń; uruchamia je usługa Brokera (bilet startowy na stdin).

use std::process::ExitCode;
use std::sync::Arc;

use app_safety::ui::{read_ticket, run};
use platform_contract::StopSignal;
use platform_windows_impl::{WinApprovalSurface, WinKernel};
use watchdog_contract::SystemClock;

fn main() -> ExitCode {
    let ticket = read_ticket(std::io::stdin().lock());
    let result = ticket.and_then(|t| {
        let surface = Arc::new(WinApprovalSurface::new());
        run(
            &t,
            &WinKernel,
            &WinKernel,
            surface,
            &SystemClock,
            &StopSignal::new(),
        )
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[alfa-broker-ui] {e}");
            ExitCode::FAILURE
        }
    }
}
