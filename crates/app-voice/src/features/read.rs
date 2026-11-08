//! Czytanie na głos (`voice-readaloud`): zaznaczenie albo dokument okna na pierwszym planie (UIA
//! `TextPattern` tylko do odczytu; zapas Ctrl+C z przywróceniem schowka) albo schowek → głos
//! bieżącej agentki (TTS prywatny), **kolejka** kolejnych zleceń, tempo 0,5–2,0, stop (`Esc`,
//! „stop” głosem, kill-switch). Tekst jest niezaufany: przechwycony przy zleceniu, żyje tylko
//! w pamięci zadania czytania (stop czyści kolejkę), nie trafia do zdarzeń ani do modelu —
//! w trakcie czytania tury głosowe nie idą do czatu ([`super::F5::voice_busy`]).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_api::dto::{LocalizedText, ReadAction, ReadControlAction, ReadSource, VoiceFeatures};
use app_api::error::{AppError, ErrorCode};
use personas_contract::PersonaId;
use platform_contract::{ClipboardContent, ClipboardPort};
use serde_json::json;
use tokio::sync::mpsc;
use voice_readaloud_contract::{
    ReadAloud, ReadAloudCfg, ReadAloudError, ReadAloudEvent, ReadControl, ReadPhase, ReadScope,
    ReadStatus, RefuseReason, SourceText, TextSource, UntrustedText,
};
use voice_readaloud_impl::{CopyFallback, ReadAloudParts, ReadAloudService, UiaTextSource};

use super::settings::{self, READ_RATE, clamp_rate};
use super::{F5, view};
use crate::engine::Pacer;
use crate::port::Voice;

/// Skrót globalny „czytaj zaznaczenie” (rejestruje powłoka; R nie jest literą polską — AltGr).
pub const SHORTCUT: &str = "Ctrl+Alt+R";
/// Rytm pętli czytania.
const TICK: Duration = Duration::from_millis(20);
/// Najwięcej zleceń w kolejce.
const MAX_QUEUE: usize = 16;

/// Sterowanie czytaniem.
pub(crate) enum ReadCmd {
    Control(ReadControl),
    Enqueue(Box<SourceText>),
}

/// Stan czytania.
#[derive(Default)]
pub(crate) struct ReadRt {
    pub ctl: Option<mpsc::UnboundedSender<ReadCmd>>,
    pub starting: bool,
    pub status: Option<ReadStatus>,
    pub queued: usize,
    pub refused: Option<LocalizedText>,
}

impl ReadRt {
    /// Czytanie trwa (albo startuje).
    pub fn active(&self) -> bool {
        self.starting || self.ctl.as_ref().is_some_and(|c| !c.is_closed())
    }
}

/// Tekst przechwycony przy zleceniu — źródło usługi (bez ponownego odczytu okna).
struct Captured(Mutex<Option<SourceText>>);

impl TextSource for Captured {
    fn read(&self, _scope: ReadScope, _max_chars: usize) -> Result<SourceText, ReadAloudError> {
        self.0
            .lock()
            .ok()
            .and_then(|mut t| t.take())
            .ok_or(ReadAloudError::Refused(RefuseReason::NoText))
    }
}

/// Schowek jako źródło (tylko tekst; ucięty do `max_chars`).
pub(crate) fn clipboard_text(
    clipboard: &dyn ClipboardPort,
    max_chars: usize,
) -> Result<SourceText, ReadAloudError> {
    match clipboard.get() {
        Ok(ClipboardContent::Text(t)) if !t.trim().is_empty() => {
            let truncated = t.chars().count() > max_chars;
            let text: String = t.chars().take(max_chars).collect();
            Ok(SourceText {
                text: UntrustedText::new(text),
                app: "Schowek".into(),
                scope: ReadScope::Selection,
                truncated,
            })
        }
        Ok(_) => Err(ReadAloudError::Refused(RefuseReason::NoText)),
        Err(e) => Err(ReadAloudError::Platform(e.to_string())),
    }
}

fn control_of(c: ReadControlAction) -> ReadControl {
    match c {
        ReadControlAction::Pause => ReadControl::Pause,
        ReadControlAction::Resume => ReadControl::Resume,
        ReadControlAction::Next => ReadControl::Next,
        ReadControlAction::Previous => ReadControl::Previous,
        ReadControlAction::Faster => ReadControl::Faster,
        ReadControlAction::Slower => ReadControl::Slower,
        ReadControlAction::Restart => ReadControl::Restart,
        ReadControlAction::Stop => ReadControl::Stop,
    }
}

impl F5 {
    /// Steruje trwającym czytaniem; `false` — nic nie jest czytane.
    pub(crate) fn control_reading(&self, control: ReadControl) -> bool {
        self.lock()
            .read
            .ctl
            .as_ref()
            .is_some_and(|c| c.send(ReadCmd::Control(control)).is_ok())
    }

    /// Stop czytania z kolejką (`Esc`, „stop”, kill-switch).
    pub(crate) fn stop_reading(&self) {
        self.control_reading(ReadControl::Stop);
    }
}

fn unavailable(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::Unavailable, message)
}

impl Voice {
    /// `voice_read`: start (zaznaczenie / dokument / schowek; w trakcie — do kolejki),
    /// sterowanie, tempo.
    pub(crate) async fn read_action(
        self: &Arc<Self>,
        action: ReadAction,
    ) -> Result<VoiceFeatures, AppError> {
        self.f5.ensure_loaded().await;
        let result = match action {
            ReadAction::Start { source } => self.read_start(source).await,
            ReadAction::Control { control } => {
                let sent = self.f5.control_reading(control_of(control));
                if sent || control == ReadControlAction::Stop {
                    Ok(())
                } else {
                    Err(AppError::invalid("Nic nie jest teraz czytane."))
                }
            }
            ReadAction::SetRate { rate } => self.read_rate(rate).await,
        };
        self.f5.publish();
        result.map(|()| self.f5.view())
    }

    async fn read_rate(&self, rate: f64) -> Result<(), AppError> {
        let rate = clamp_rate(rate as f32);
        settings::save(self.f5.deps.config.as_deref(), READ_RATE, json!(rate))
            .await
            .map_err(AppError::internal)?;
        let current = {
            let mut st = self.f5.lock();
            st.settings.read_rate = rate;
            st.read.status.as_ref().map(|s| s.rate)
        };
        // W trakcie czytania: tyle kroków tempa (0,1), ile trzeba do nowej wartości.
        if let Some(cur) = current {
            let steps = ((rate - cur) / ReadAloudCfg::default().rate_step).round() as i32;
            let control = if steps > 0 {
                ReadControl::Faster
            } else {
                ReadControl::Slower
            };
            for _ in 0..steps.unsigned_abs() {
                self.f5.control_reading(control);
            }
        }
        Ok(())
    }

    /// Przechwycenie tekstu teraz (okno na pierwszym planie albo schowek).
    async fn capture(&self, source: ReadSource) -> Result<SourceText, ReadAloudError> {
        let max = ReadAloudCfg::default().max_chars;
        let desk = self.f5.deps.desktop.clone();
        let Some(desk) = desk else {
            return Err(ReadAloudError::Refused(RefuseReason::Unsupported));
        };
        if source == ReadSource::Clipboard {
            let clip = desk
                .clipboard
                .ok_or(ReadAloudError::Refused(RefuseReason::Unsupported))?;
            return clipboard_text(clip.as_ref(), max);
        }
        let mut src = UiaTextSource::new(desk.desktop.clone(), desk.uia.clone());
        if let Some(clipboard) = desk.clipboard.clone() {
            src = src.with_copy_fallback(CopyFallback {
                input: desk.input.clone(),
                clipboard,
            });
        }
        let scope = if source == ReadSource::Document {
            ReadScope::Document
        } else {
            ReadScope::Selection
        };
        // UIA i schowek blokują (limity portu ≤ 5 s) — poza wątkiem asynchronicznym.
        tokio::task::spawn_blocking(move || src.read(scope, max))
            .await
            .map_err(|e| ReadAloudError::Platform(e.to_string()))?
    }

    fn refuse(&self, e: &ReadAloudError) -> AppError {
        let text = match e {
            ReadAloudError::Refused(r) => view::read_refusal(*r),
            other => LocalizedText::new(format!("Czytanie: {other}"), "Reading failed."),
        };
        {
            let mut st = self.f5.lock();
            st.read.refused = Some(text.clone());
            st.read.starting = false;
        }
        match e {
            ReadAloudError::Refused(_) => AppError::forbidden(text.pl),
            _ => unavailable(text.pl),
        }
    }

    async fn read_start(self: &Arc<Self>, source: ReadSource) -> Result<(), AppError> {
        let parts = self
            .f5
            .parts()
            .ok_or_else(|| AppError::unavailable("Czytanie na głos", "voice-readaloud"))?;
        let text = match self.capture(source).await {
            Ok(t) => t,
            Err(e) => {
                let refused = ReadAloudEvent::Refused {
                    reason: match e {
                        ReadAloudError::Refused(r) => r,
                        _ => RefuseReason::Unsupported,
                    },
                };
                self.publish_bus(vec![refused.to_bus_event()]).await;
                return Err(self.refuse(&e));
            }
        };
        let queued = {
            let mut st = self.f5.lock();
            st.read.refused = None;
            match st.read.ctl.as_ref().filter(|c| !c.is_closed()) {
                Some(_) if st.read.queued >= MAX_QUEUE => {
                    return Err(AppError::invalid(format!(
                        "Kolejka czytania jest pełna ({MAX_QUEUE})."
                    )));
                }
                Some(c) => {
                    let sent = c.send(ReadCmd::Enqueue(Box::new(text.clone()))).is_ok();
                    if sent {
                        st.read.queued += 1;
                    }
                    sent
                }
                None => {
                    st.read.starting = true;
                    false
                }
            }
        };
        if queued {
            return Ok(());
        }
        let audio = match parts.read_audio() {
            Ok(a) => a,
            Err(e) => {
                self.f5.lock().read.starting = false;
                return Err(unavailable(format!("Czytanie na głos: {e}")));
            }
        };
        let rate = self.f5.lock().settings.read_rate;
        let captured = Arc::new(Captured(Mutex::new(Some(text))));
        let mut service = ReadAloudService::new(
            ReadAloudParts {
                source: captured.clone(),
                tts: audio.tts,
                output: audio.output,
                scheduler: parts.scheduler(),
            },
            ReadAloudCfg {
                rate,
                ..ReadAloudCfg::default()
            },
        );
        let persona = PersonaId::parse(&self.agent()).unwrap_or_else(PersonaId::alfa);
        let started = service.start(ReadScope::Selection, persona).await;
        let events = service.take_events();
        self.publish_bus(events.iter().map(ReadAloudEvent::to_bus_event).collect())
            .await;
        let status = started.map_err(|e| self.refuse(&e))?;
        let (tx, rx) = mpsc::unbounded_channel();
        {
            let mut st = self.f5.lock();
            st.read.ctl = Some(tx);
            st.read.starting = false;
            st.read.status = Some(status);
            st.read.queued = 0;
        }
        let me = self.clone();
        let pacer = parts.pacer(TICK);
        tokio::spawn(async move { me.read_loop(service, captured, rx, pacer).await });
        Ok(())
    }

    async fn read_loop(
        self: Arc<Self>,
        mut service: ReadAloudService,
        captured: Arc<Captured>,
        mut rx: mpsc::UnboundedReceiver<ReadCmd>,
        mut pacer: Box<dyn Pacer>,
    ) {
        let mut queue: VecDeque<SourceText> = VecDeque::new();
        loop {
            pacer.tick().await;
            let mut stopped = false;
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    ReadCmd::Control(ReadControl::Stop) => {
                        service.control(ReadControl::Stop).await;
                        queue.clear();
                        stopped = true;
                    }
                    ReadCmd::Control(c) => {
                        service.control(c).await;
                    }
                    ReadCmd::Enqueue(t) => queue.push_back(*t),
                }
            }
            let mut status = if stopped {
                service.status()
            } else {
                service.step().await
            };
            let done = matches!(status.phase, ReadPhase::Idle | ReadPhase::Finished);
            if done
                && !stopped
                && let Some(next) = queue.pop_front()
            {
                if let Ok(mut slot) = captured.0.lock() {
                    *slot = Some(next);
                }
                let persona = PersonaId::parse(&self.agent()).unwrap_or_else(PersonaId::alfa);
                if let Ok(s) = service.start(ReadScope::Selection, persona).await {
                    status = s;
                }
            }
            let events = service.take_events();
            self.publish_bus(events.iter().map(ReadAloudEvent::to_bus_event).collect())
                .await;
            let finished =
                matches!(status.phase, ReadPhase::Idle | ReadPhase::Finished) && queue.is_empty();
            {
                let mut st = self.f5.lock();
                st.read.status = Some(status);
                st.read.queued = queue.len();
            }
            self.f5.publish();
            if finished || stopped {
                break;
            }
        }
        // Treść czytana (usługa, kolejka) znika z pamięci razem z zadaniem.
        drop(service);
        {
            let mut st = self.f5.lock();
            st.read.ctl = None;
            st.read.queued = 0;
            st.read.status = None;
        }
        self.f5.publish();
    }
}
