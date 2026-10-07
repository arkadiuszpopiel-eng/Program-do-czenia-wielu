//! `alfa-fake-agent-cli` — fałszywe CLI do testów mostów (bez prawdziwych `claude`/`codex`):
//! - `--version` → [`io::FAKE_VERSION`];
//! - `app-server` → atrapa `codex app-server` (JSON-RPC po stdio);
//! - `-p ...` → atrapa `claude -p --output-format stream-json --input-format stream-json`;
//! - `--alfa-fake-sleep` → długi sen (proces-wnuk do testu zabijania drzewa).
//!
//! Zachowanie wybiera znacznik scenariusza w poleceniu (`[alfa-fake:<nazwa>]`).

mod claude;
mod codex;
mod io;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = if args.iter().any(|a| a == "--alfa-fake-sleep") {
        std::thread::sleep(std::time::Duration::from_secs(600));
        0
    } else if args.iter().any(|a| a == "--version") {
        io::raw(io::FAKE_VERSION);
        0
    } else if args.first().map(String::as_str) == Some("app-server") {
        codex::run()
    } else if args.iter().any(|a| a == "-p" || a == "--print") {
        claude::run(&args)
    } else {
        eprintln!("alfa-fake-agent-cli: nieznany tryb");
        2
    };
    ExitCode::from(code)
}
