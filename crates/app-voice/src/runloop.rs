//! Pętla potoku (jedno zadanie — potok nie jest `Sync`): krok co `tick_ms`, pigułka (kto mówi,
//! poziom, transkrypt częściowy) i stan trybu głosowego; „stop wszystko” i „anuluj” z komend
//! głosowych idą do czatu (kill-switch, anulowanie zadania agentki). F5: uzbrajanie słów
//! wywoławczych, test wykrycia (bez otwierania rozmowy), DND, „stop”/„pauza” dla czytania.

use std::sync::{Arc, Mutex};

use app_api::dto::{AlfaEvent, LocalizedText, ToastKind};
use app_api::events::EventHub;
use app_api::ports::VoiceChat;
use core_bus_contract::Event;
use tokio::sync::mpsc;
use voice_pipeline_contract::{
    EVENT_CANCEL_TASK, EVENT_DEGRADED, EVENT_KILL_SWITCH, EVENT_PERSONA_SWITCHED, PipelineInput,
};
use voice_readaloud_contract::ReadControl;

use crate::engine::VoiceEngine;
use crate::features::F5;
use crate::pill;
use crate::port::{Ctl, Shared, lock};

/// Co ile ms pigułka bez zmian (sam poziom), co ile `MicLevel` (≤ 30/s).
const PILL_EVERY_MS: u64 = 100;
const LEVEL_EVERY_MS: u64 = 34;
/// Najdłuższe czekanie na wygaszenie potoku po wyłączeniu mikrofonu.
const STOP_GRACE_MS: u64 = 2_000;
/// Co ile ms odświeżać liczniki nasłuchu (test słowa wywoławczego).
const STATS_EVERY_MS: u64 = 250;

/// Pętla potoku.
pub(crate) struct Loop {
    pub engine: VoiceEngine,
    pub ctl: mpsc::UnboundedReceiver<Ctl>,
    pub bus: mpsc::UnboundedReceiver<Event>,
    pub events: EventHub,
    pub chat: Arc<dyn VoiceChat>,
    pub shared: Arc<Mutex<Shared>>,
    pub f5: Arc<F5>,
}

impl Loop {
    pub(crate) async fn run(mut self) {
        let mut stopping: Option<u64> = None;
        let mut last_pill: Option<(u64, app_api::dto::VoicePillState)> = None;
        let mut last_level = 0u64;
        let mut last_stats = 0u64;
        loop {
            self.engine.pacer.tick().await;
            while let Ok(ctl) = self.ctl.try_recv() {
                match ctl {
                    Ctl::Input(input) => self.engine.pipeline.input(input),
                    Ctl::Stop => stopping = stopping.or(Some(self.engine.pipeline.status().now_ms)),
                    Ctl::ArmWake(listener) => self.arm(*listener),
                    Ctl::DisarmWake => {
                        self.engine.pipeline.disarm_wake_words();
                        self.f5.lock().wake.armed = false;
                        self.f5.publish();
                    }
                }
            }
            self.engine.pipeline.step().await;
            while let Ok(event) = self.bus.try_recv() {
                self.on_event(&event);
            }
            let status = self.engine.pipeline.status();
            let now = status.now_ms;
            let pill = pill::pill(&status);
            let due = last_pill
                .as_ref()
                .is_none_or(|(at, prev)| pill::changed(prev, &pill) || now >= at + PILL_EVERY_MS);
            if due {
                self.events.emit(AlfaEvent::VoicePill {
                    state: pill.clone(),
                });
                last_pill = Some((now, pill));
            }
            if status.mic_open && now >= last_level + LEVEL_EVERY_MS {
                last_level = now;
                self.events.emit(AlfaEvent::MicLevel {
                    level: pill::level(status.level_db),
                });
            }
            if now >= last_stats + STATS_EVERY_MS {
                last_stats = now;
                if self.stats(now) {
                    stopping = stopping.or(Some(now));
                }
            }
            let idle = status.phase == voice_dialog_contract::DialogPhase::Idle && !status.mic_open;
            if let Some(since) = stopping
                && (idle || now >= since + STOP_GRACE_MS)
            {
                break;
            }
        }
        self.engine.pipeline.disarm_wake_words();
        {
            let mut s = lock(&self.shared);
            s.ctl = None;
            s.conversation = false;
        }
        {
            let mut st = self.f5.lock();
            st.wake.armed = false;
            st.wake.listening = false;
        }
        self.f5.publish();
        self.events.emit(AlfaEvent::VoicePill {
            state: app_api::dto::VoicePillState {
                agent: lock(&self.shared).agent.clone(),
                mic: app_api::dto::MicState::Off,
                level: 0.0,
                speaker: app_api::dto::VoiceSpeaker::Nobody,
                partial: None,
            },
        });
    }

    fn arm(&mut self, listener: voice_wake_contract::WakeWordListener) {
        let result = self.engine.pipeline.arm_wake_words(listener);
        {
            let mut st = self.f5.lock();
            st.wake.armed = result.is_ok();
            st.wake.error = result.as_ref().err().map(ToString::to_string);
        }
        if let Err(e) = result {
            self.events.emit(AlfaEvent::Toast {
                kind: ToastKind::Warning,
                message: LocalizedText::new(
                    format!("Słowa wywoławcze: {e}"),
                    "Wake words could not start.",
                ),
            });
        }
        self.f5.publish();
    }

    /// Liczniki nasłuchu (test, odrzucenia bramki właściciela) i koniec okna testu; `true` —
    /// test się skończył, słowa są wyłączone i nie trwa rozmowa (potok można zatrzymać).
    fn stats(&mut self, now: u64) -> bool {
        let stats = self.engine.pipeline.wake_stats();
        let conversation = lock(&self.shared).conversation;
        let mut st = self.f5.lock();
        if let Some(s) = stats {
            st.wake.test.owner_rejected = u32::try_from(s.owner_rejected).unwrap_or(u32::MAX);
        }
        st.wake.now_ms = now;
        if st.wake.test.active && st.wake.test_until_ms.is_none() {
            st.wake.test_until_ms = Some(now + crate::features::wake::TEST_MS);
        }
        let expired = st.wake.test.active && st.wake.test_until_ms.is_some_and(|t| now >= t);
        let mut stop = false;
        if expired {
            st.wake.test.active = false;
            st.wake.test_until_ms = None;
            if !st.settings.wake_enabled && st.wake.armed {
                self.engine.pipeline.disarm_wake_words();
                st.wake.armed = false;
                stop = !conversation;
            }
        }
        drop(st);
        self.f5.publish();
        stop
    }

    fn on_event(&mut self, event: &Event) {
        let chat = self.chat.clone();
        match event.kind.as_str() {
            EVENT_KILL_SWITCH => {
                self.f5.stop_reading();
                self.f5.stop_dictation();
                tokio::spawn(async move { chat.kill_switch().await });
            }
            EVENT_CANCEL_TASK => {
                tokio::spawn(async move { chat.cancel_task().await });
            }
            EVENT_PERSONA_SWITCHED => {
                if let Some(to) = event.payload.get("to").and_then(|v| v.as_str()) {
                    lock(&self.shared).agent = to.to_owned();
                    self.f5.lock().agent = to.to_owned();
                }
            }
            EVENT_DEGRADED => {
                let reason = event
                    .payload
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("składnik głosu niedostępny");
                self.events.emit(AlfaEvent::Toast {
                    kind: ToastKind::Warning,
                    message: LocalizedText::new(
                        format!("Głos: {reason}"),
                        "Voice: a component is degraded.",
                    ),
                });
            }
            voice_wake_contract::EVENT_LISTEN_START => self.on_listen_start(event),
            voice_wake_contract::EVENT_LISTEN_STOP => {
                self.f5.lock().wake.listening = false;
                self.f5.publish();
            }
            voice_wake_contract::EVENT_DND => {
                let on = event.payload.get("on").and_then(|v| v.as_bool());
                if let Some(on) = on {
                    self.f5.lock().wake.dnd = on;
                    self.f5.publish();
                }
            }
            voice_cmd_contract::EVENT_DETECTED | voice_cmd_contract::EVENT_IGNORED => {
                self.on_command(event);
            }
            _ => {}
        }
    }

    /// Wykrycie słowa wywoławczego: w trybie testu tylko licznik (rozmowa się nie otwiera).
    fn on_listen_start(&mut self, event: &Event) {
        let by_word = event.payload.get("source").and_then(|v| v.as_str()) == Some("wake_word");
        let agent = event
            .payload
            .get("addressed")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let test = {
            let mut st = self.f5.lock();
            st.wake.listening = true;
            let test = by_word && st.wake.test.active;
            if test {
                st.wake.test.detections += 1;
                st.wake.test.last_agent = agent;
            }
            test
        };
        if test {
            let muted = lock(&self.shared).muted;
            self.engine.pipeline.input(PipelineInput::Deactivate);
            self.engine
                .pipeline
                .input(PipelineInput::SetMuted { muted: true });
            self.engine
                .pipeline
                .input(PipelineInput::SetMuted { muted });
        }
        self.f5.publish();
    }

    /// „Stop” / „pauza” / „dalej” wypowiedziane w trakcie czytania sterują czytaniem.
    fn on_command(&mut self, event: &Event) {
        let command = event
            .payload
            .get("command")
            .and_then(|c| c.get("command").or(Some(c)))
            .and_then(|c| c.as_str())
            .unwrap_or_default();
        let control = match command {
            "stop" | "stop_all" | "cancel" => ReadControl::Stop,
            "pause" | "wait" => ReadControl::Pause,
            "resume" => ReadControl::Resume,
            _ => return,
        };
        self.f5.control_reading(control);
    }
}
