//! Okresowe zwalnianie bezczynnych modułów (zadanie tokio).

use std::sync::Arc;
use std::time::Duration;

use core_registry_contract::Registry;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

/// Uruchamia zadanie, które co `period` woła `unload_idle` na rejestrze.
/// Zadanie kończy się dopiero przy `abort()` uchwytu (lub zamknięciu runtime).
pub fn spawn_idle_reaper<R: Registry + 'static>(
    registry: Arc<R>,
    period: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(period);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        tick.tick().await;
        loop {
            tick.tick().await;
            match registry.unload_idle().await {
                Ok(unloaded) if !unloaded.is_empty() => {
                    tracing::debug!(liczba = unloaded.len(), "zwolniono bezczynne moduły");
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "zwalnianie bezczynnych modułów nie powiodło się")
                }
            }
        }
    })
}
