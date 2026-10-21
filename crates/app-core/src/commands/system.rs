//! Komendy `system_*`: stan systemu (PLAN §14.4) i ponowienie kolejki offline.

use crate::core::AppCore;
use crate::dto::{
    AlfaEvent, DiskInfo, EventsResume, LocalizedText, MicAvailability, RateLimitInfo, SystemStatus,
    ToastKind,
};
use crate::error::AppError;

/// Próg „mało miejsca" (2 GB).
const DISK_LOW_BYTES: u64 = 2 * 1024 * 1024 * 1024;

impl AppCore {
    /// Bieżący stan systemu (dla zdarzeń `SystemStatusChanged`).
    pub(crate) async fn status_snapshot(&self) -> SystemStatus {
        let now = chrono::Utc::now();
        let (online, rate_limit) = {
            let mut rt = self.rt();
            if rt.rate_limit.as_ref().is_some_and(|(_, at)| *at <= now) {
                rt.rate_limit = None;
            }
            (rt.online, rt.rate_limit.clone())
        };
        let free = self.inner.shell.disk_free(&self.inner.paths.local);
        SystemStatus {
            online,
            queued_messages: self.queued_count(),
            rate_limit: rate_limit.map(|(provider, at)| RateLimitInfo {
                provider,
                resets_at: crate::dto::iso(at),
            }),
            keys_configured: self.inner.brain.keys_configured(),
            profile: self.default_profile().await,
            mic: if self.detected_inputs().is_empty() && self.inner.device.current().audio.is_some()
            {
                MicAvailability::Missing
            } else {
                MicAvailability::Ok
            },
            disk: DiskInfo {
                free_bytes: free.unwrap_or(0),
                low: free.is_some_and(|f| f < DISK_LOW_BYTES),
            },
        }
    }

    /// `system_status`: online, kolejka, 429, klucze, mikrofon, dysk.
    pub async fn system_status(&self) -> Result<SystemStatus, AppError> {
        Ok(self.status_snapshot().await)
    }

    /// `system_clipboard_text`: tekst ze schowka systemu dla polecenia „Wklej” z menu Alfy
    /// (WebView2 odmawia stronie odczytu schowka). Wywoływane wyłącznie po wyborze „Wklej” przez
    /// użytkownika; `None`, gdy schowek jest pusty albo nie zawiera tekstu. Treść nie trafia do
    /// logów ani zdarzeń.
    pub async fn system_clipboard_text(&self) -> Result<Option<String>, AppError> {
        use platform_contract::ClipboardContent;
        let clipboard = self.inner.clipboard.clone();
        let content = tokio::task::spawn_blocking(move || clipboard.get())
            .await
            .map_err(|e| AppError::internal(format!("schowek: {e}")))?
            .map_err(|e| AppError::internal(format!("schowek: {e}")))?;
        Ok(match content {
            ClipboardContent::Text(text) => Some(text),
            _ => None,
        })
    }

    /// `system_retry_queue`: „Ponów” zawsze wysyła kolejkę; „offline” wraca z błędu sieci (21.10).
    pub async fn system_retry_queue(&self) -> Result<(), AppError> {
        self.rt().online = true;
        // Stan łączności i kolejki także po błędzie startu sesji (baner offline nie zostaje).
        let flushed = self.flush_queue().await;
        let status = self.status_snapshot().await;
        self.emit(AlfaEvent::SystemStatusChanged { status });
        flushed
    }

    /// STOP WSZYSTKIEGO (`Ctrl+Shift+F12`, zasobnik): anuluje generacje we wszystkich sesjach
    /// (natychmiast), potem kill-switch Brokera (tokeny, drzewa procesów, cisza audio) i stop mowy;
    /// czeka na zapis przerwanych tur. Zwraca liczbę zatrzymanych generacji.
    pub async fn system_kill_all(&self, origin: crate::ports::KillOrigin) -> usize {
        // Generacje i przebiegi agentek (silnik czatu) — anulowane od razu.
        let handles = self.chat().cancel_all();
        for cancel in self.rt().downloads.values() {
            cancel.cancel();
        }
        // Zadania schedulera (agentki i mosty) — wykonawczynie przerwane od razu.
        let tasks =
            scheduler_lite_contract::SchedulerLite::kill_all(&*self.inner.tasks.scheduler());
        tracing::info!(
            zadania = tasks,
            "kill-switch: zatrzymano zadania schedulera"
        );
        // Kamery: zatrzymanie w osobnym zadaniu — zawieszony sterownik kamery nie może opóźnić
        // zatrzymania Brokera i agentek. Żądania w toku są unieważniane od razu; na zamknięcie
        // strumieni czekamy krótko na końcu (przełączników nie zmienia).
        let cameras = self.inner.cameras.hub.clone();
        let cameras_stopped = tokio::spawn(async move { cameras.stop_all().await });
        // Przeglądarki agentek (F6): drzewa procesów zamknięte, zgody hostów wygasają.
        let _ = self.inner.agents.as_ref().map(|a| a.tools.kill_switch());
        let gap = match self.inner.broker.kill_all(origin).await {
            Ok(gap) => gap,
            Err(e) => {
                tracing::error!(error = %e, "kill-switch Brokera nie powiódł się");
                Some(format!(
                    "kill-switch Brokera nie powiódł się: {}",
                    e.message
                ))
            }
        };
        if let Err(e) = self.inner.voice.stop_speech().await {
            tracing::warn!(error = %e, "stop mowy przy kill-switchu nie powiódł się");
        }
        if tokio::time::timeout(std::time::Duration::from_secs(2), cameras_stopped)
            .await
            .is_err()
        {
            tracing::warn!(
                "kill-switch: kamery zamykają się dłużej niż 2 s (zamykanie trwa w tle)"
            );
        }
        // Dyktowanie wpisuje tekst do cudzych okien — STOP WSZYSTKIEGO musi je też zakończyć
        // (wcześniej zatrzymywała je tylko komenda głosowa). Brak aktywnego dyktowania to nie błąd.
        if let Err(e) = self
            .voice_dictation(crate::dto::DictationAction::Stop)
            .await
        {
            tracing::debug!(error = %e.message, "kill-switch: dyktowanie nie było aktywne");
        }
        for h in &handles {
            h.wait(std::time::Duration::from_secs(5)).await;
        }
        // Audyt A-03: „zatrzymano” tylko wtedy, gdy Broker potwierdził; inaczej trwały komunikat
        // o zatrzymaniu niepełnym (praca w Alfie przerwana, ale nie wszystko po stronie Brokera).
        let message = match &gap {
            None => LocalizedText::new(
                "STOP WSZYSTKIEGO: zatrzymano pracę agentek.",
                "STOP EVERYTHING: agents stopped.",
            ),
            Some(what) => {
                tracing::error!(brak = %what, "kill-switch: zatrzymanie niepełne");
                LocalizedText::new(
                    format!(
                        "STOP WSZYSTKIEGO: praca agentek w Alfie przerwana, ale zatrzymanie jest                          NIEPEŁNE — {what}. Sprawdź Ustawienia → Bezpieczeństwo."
                    ),
                    format!(
                        "STOP EVERYTHING: agents stopped in Alfa, but the stop is INCOMPLETE —                          {what}. Check Settings → Security."
                    ),
                )
            }
        };
        let kind = if gap.is_some() {
            ToastKind::Error
        } else {
            ToastKind::Warning
        };
        self.emit(AlfaEvent::Toast { kind, message });
        handles.len()
    }
}

impl AppCore {
    /// `events_resume`: ramki zdarzeń po `after_seq` dla klienta, który zgubił paczki (zaległy
    /// odbiorca, zerwane łącze). `complete = false` — brakujących ramek nie ma już w buforze
    /// albo klient pamięta numery z innego uruchomienia; wtedy wczytuje stan od nowa.
    pub async fn events_resume(
        &self,
        after_seq: u64,
        epoch: Option<String>,
    ) -> Result<EventsResume, AppError> {
        Ok(self.inner.events.resume(after_seq, epoch.as_deref()))
    }
}
