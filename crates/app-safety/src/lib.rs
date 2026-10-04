//! Korzeń kompozycji procesów Jądra bezpieczeństwa (F3, część 2; ADR 3, PLAN §8.1–8.2, §8.6).
//!
//! - [`broker`]: `alfa-broker` — usługa Windows (dyspozytor SCM z `platform-windows-impl`) albo
//!   tryb konsolowy `--console` (deweloperski): Audyt w katalogu prywatnym z kotwicą, silnik
//!   Brokera, serwer IPC na named pipe z ACL i wiązaniem ról z tożsamością procesu, nadzór
//!   Broker-UI w sesji użytkownika z wysoką integralnością.
//! - [`ui`]: `alfa-broker-ui` — bilet ze stdin → łącze z Brokerem (sprawdzenie konta serwera) →
//!   natywne okno zatwierdzeń.
//! - [`watchdog`]: `alfa-watchdog` — `Ctrl+Shift+F12` → kill-switch: drzewa procesów (Job Objects)
//!   od razu, Broker przez IPC z limitem 100 ms.
//! - [`ChildLauncher`]: uruchamianie „jak wywołujący” (tryb deweloperski, przenośne).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod broker;
mod child;
pub mod ui;
pub mod watchdog;

pub use child::ChildLauncher;

/// Wartość argumentu `--nazwa wartość`.
pub fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

/// Czy podano przełącznik `--nazwa`.
pub fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// „Linia życia” procesu uruchomionego przez aplikację (`--lifeline`): wątek czyta stdin do
/// końca i zgłasza zatrzymanie, gdy aplikacja go zamknie (także po jej awarii) — proces Jądra
/// trybu przenośnego nie zostaje sierotą trzymającą nazwę potoku.
pub fn spawn_lifeline(stop: platform_contract::StopSignal) {
    let spawned = std::thread::Builder::new()
        .name("alfa-lifeline".into())
        .spawn(move || {
            let mut sink = Vec::new();
            let _ = std::io::Read::read_to_end(&mut std::io::stdin().lock(), &mut sink);
            stop.stop();
        });
    if let Err(e) = spawned {
        eprintln!("[alfa] linia życia niedostępna: {e}");
    }
}
