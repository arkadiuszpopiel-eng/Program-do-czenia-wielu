//! Atrapa modułu-wzorca „echo” (docs/PLAN.md §4.5).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use example_module_contract::{Echo, EchoError, EchoReply, validate_input};

#[derive(Debug, Default)]
struct State {
    calls: Vec<String>,
    seq: u64,
    fail_next: Option<EchoError>,
}

/// Deterministyczne echo bez magistrali; rejestruje wywołania.
#[derive(Debug, Default)]
pub struct FakeEcho {
    state: Mutex<State>,
}

impl FakeEcho {
    /// Nowa atrapa.
    pub fn new() -> Self {
        Self::default()
    }

    /// Wszystkie wejścia przekazane do `echo` (także odrzucone).
    pub fn calls(&self) -> Vec<String> {
        self.lock().calls.clone()
    }

    /// Następne wywołanie zwróci ten błąd (jednorazowo) — do testów ścieżek błędów.
    pub fn fail_next(&self, error: EchoError) {
        self.lock().fail_next = Some(error);
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[async_trait]
impl Echo for FakeEcho {
    async fn echo(&self, input: &str) -> Result<EchoReply, EchoError> {
        let mut st = self.lock();
        st.calls.push(input.to_owned());
        if let Some(err) = st.fail_next.take() {
            return Err(err);
        }
        let chars = validate_input(input)?;
        st.seq += 1;
        Ok(EchoReply {
            text: input.to_owned(),
            chars,
            seq: st.seq,
        })
    }
}
