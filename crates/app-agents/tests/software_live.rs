//! Wykrywanie programów na prawdziwym komputerze (ręcznie: `cargo test -p app-agents --test
//! software_live -- --ignored --nocapture`) — pokazuje opis, który dostaje agentka.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use tools_system_contract::{SoftwareProbe, describe};
use tools_system_impl::CliSoftwareProbe;

#[test]
#[ignore = "prawdziwy komputer: uruchamia `--version` zainstalowanych programów"]
fn real_machine_description() {
    let probe = CliSoftwareProbe::new(Arc::new(accounts_hub_impl::SystemCliProbe::from_env()));
    let started = std::time::Instant::now();
    let items = probe.scan();
    eprintln!("{} programów w {:?}", items.len(), started.elapsed());
    eprintln!("{}", describe(&items, &[]));
    assert!(!items.is_empty());
}
