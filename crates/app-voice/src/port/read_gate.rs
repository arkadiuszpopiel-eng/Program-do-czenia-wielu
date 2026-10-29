//! Bramka słuchania na czas czytania na głos. Czytanie i próbka głosu grają poza rozmową (bez
//! tłumienia echa): przy włączonym mikrofonie wracałyby jako wypowiedź użytkownika (próba toru
//! akustycznego, 2026-10-12). Na czas odtwarzania słuchanie jest wstrzymane.

use std::sync::Arc;
use std::time::Duration;

use voice_pipeline_contract::PipelineInput;

use super::{Ctl, Voice, lock};

/// Krok sprawdzania, pogłos po końcu, górny limit.
const READ_GATE_STEP: Duration = Duration::from_millis(100);
const READ_GATE_TAIL: Duration = Duration::from_millis(400);
const READ_GATE_MAX: Duration = Duration::from_secs(120);

impl Voice {
    /// Wycisza słuchanie, gdy trwa rozmowa; `true` — po czytaniu trzeba je przywrócić.
    pub(super) fn mute_for_reading(&self) -> bool {
        let gate = self.running().is_some() && !lock(&self.shared).muted;
        if gate {
            self.ctl(Ctl::Input(PipelineInput::SetMuted { muted: true }));
        }
        gate
    }

    /// Przywraca słuchanie po końcu czytania i pogłosie pokoju.
    pub(super) fn unmute_after_reading(self: &Arc<Self>) {
        let voice = self.clone();
        tokio::spawn(async move {
            let deadline = tokio::time::Instant::now() + READ_GATE_MAX;
            tokio::time::sleep(READ_GATE_STEP).await;
            while voice.base.reading().await && tokio::time::Instant::now() < deadline {
                tokio::time::sleep(READ_GATE_STEP).await;
            }
            // Pogłos pokoju po ostatnim słowie.
            tokio::time::sleep(READ_GATE_TAIL).await;
            // Użytkownik mógł w tym czasie sam wyciszyć mikrofon — wtedy zostaje wyciszony.
            if !lock(&voice.shared).muted {
                voice.ctl(Ctl::Input(PipelineInput::SetMuted { muted: false }));
            }
        });
    }
}
