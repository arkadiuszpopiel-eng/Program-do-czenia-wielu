//! `alfa-watchdog` — kill-switch `Ctrl+Shift+F12` niezależny od UI i Brokera.
//! `alfa-watchdog [--broker-pipe NAZWA] [--broker-user SID] [-- <jądro> argumenty…]`.

use std::process::ExitCode;

use app_safety::watchdog::{WatchdogArgs, run};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match WatchdogArgs::parse(&args).and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[alfa-watchdog] {e}");
            ExitCode::FAILURE
        }
    }
}
