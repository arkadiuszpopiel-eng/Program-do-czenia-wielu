//! Słowa wywoławcze „Hej Alfa/Beta/Gama/Delta” w aplikacji (`voice-wake` v1): **włączane tylko
//! jawnie** (domyślnie wyłączone; PTT i przełącznik działają zawsze), z bramką właściciela
//! (`voice-speaker`, fail-closed bez profilu). Bez pomiaru FAR/FRR na korpusie (runner
//! `alfa-wake-eval`) włączenie wymaga potwierdzenia „na własne ryzyko”. Wykrycie → potok przechodzi
//! w słuchanie dla właściwej agentki; „nie przeszkadzać” i wyciszenie wstrzymują nasłuch.
//! Test wykrycia liczy trafienia bez otwierania rozmowy (`runloop.rs`).

use app_api::dto::{VoiceFeatures, WakeAction, WakeTestView};
use app_api::error::{AppError, ErrorCode};
use personas_contract::builtin_personas;
use serde_json::json;
use voice_pipeline_contract::PipelineInput;
use voice_speaker_contract::EnrollmentStatus;
use voice_speaker_impl::SpeakerOwnerCheck;
use voice_wake_contract::{KwsParams, WakeWordCfg, WakeWordListener};

use super::settings::{self, WAKE_ACCEPT_RISK, WAKE_ENABLED, WAKE_OWNER_GATE};
use super::view;
use crate::port::{Ctl, Voice, lock};

/// Próg detektora bez pomiaru (wartość startowa SPEC `voice-wake`).
pub const DEFAULT_THRESHOLD: f32 = 0.8;
/// Jak długo trwa test wykrycia (ms czasu potoku).
pub const TEST_MS: u64 = 30_000;

/// Stan nasłuchu.
#[derive(Debug, Default)]
pub(crate) struct WakeRt {
    /// Nasłuch uzbrojony w potoku.
    pub armed: bool,
    /// Trwa słuchanie po wybudzeniu (albo PTT/przełącznik).
    pub listening: bool,
    /// „Nie przeszkadzać”.
    pub dnd: bool,
    /// Mikrofon wyciszony.
    pub muted: bool,
    /// Wstrzymany na czas dyktowania albo nagrywania rejestracji.
    pub suspended: bool,
    /// Test wykrycia.
    pub test: WakeTestView,
    /// Koniec testu (czas potoku; ustawia pętla przy pierwszym odczycie).
    pub test_until_ms: Option<u64>,
    /// Czas potoku (ms) z ostatniego odczytu liczników.
    pub now_ms: u64,
    /// Ostatni błąd uzbrajania.
    pub error: Option<String>,
}

fn unavailable(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::Unavailable, message)
}

impl Voice {
    /// `voice_wake`: konfiguracja, test, „nie przeszkadzać”.
    pub(crate) async fn wake_action(&self, action: WakeAction) -> Result<VoiceFeatures, AppError> {
        self.f5.ensure_loaded().await;
        let result = match action {
            WakeAction::Configure {
                enabled,
                accept_risk,
                owner_gate,
            } => self.wake_configure(enabled, accept_risk, owner_gate).await,
            WakeAction::Test { on } => self.wake_test(on),
            WakeAction::SetDnd { on } => {
                self.f5.lock().wake.dnd = on;
                self.ctl(Ctl::Input(PipelineInput::SetDoNotDisturb { on }));
                Ok(())
            }
        };
        self.f5.publish();
        result.map(|()| self.f5.view())
    }

    /// Warunki jawnego włączenia: pomiar FAR/FRR albo świadome ryzyko; bramka właściciela
    /// wymaga zarejestrowanego głosu.
    pub(crate) fn wake_check_enable(
        &self,
        accept_risk: bool,
        owner_gate: bool,
    ) -> Result<(), AppError> {
        let parts = self
            .f5
            .parts()
            .ok_or_else(|| AppError::unavailable("Słowa wywoławcze", "voice-wake"))?;
        self.f5.refresh();
        if !parts.has_wake_model() {
            return Err(unavailable(view::NO_KWS_MODEL));
        }
        let trusted = parts.wake_calibration().is_some_and(|c| c.trusted());
        if !trusted && !accept_risk {
            return Err(AppError::forbidden(
                "Słowa wywoławcze są nieskalibrowane (brak pomiaru FAR/FRR na korpusie) — \
                 potwierdź, że włączasz je na własne ryzyko.",
            ));
        }
        if owner_gate {
            let enrolled = self
                .f5
                .verifier()
                .is_ok_and(|v| matches!(v.status(), EnrollmentStatus::Enrolled { .. }));
            if !enrolled {
                return Err(AppError::invalid(
                    "Bramka właściciela wymaga zarejestrowanego głosu — najpierw przejdź \
                     kreator „Rozpoznawanie mojego głosu” albo wyłącz bramkę.",
                ));
            }
        }
        Ok(())
    }

    async fn wake_configure(
        &self,
        enabled: bool,
        accept_risk: bool,
        owner_gate: bool,
    ) -> Result<(), AppError> {
        if enabled {
            self.wake_check_enable(accept_risk, owner_gate)?;
        }
        let config = self.f5.deps.config.clone();
        for (key, value) in [
            (WAKE_ENABLED, enabled),
            (WAKE_ACCEPT_RISK, accept_risk),
            (WAKE_OWNER_GATE, owner_gate),
        ] {
            settings::save(config.as_deref(), key, json!(value))
                .await
                .map_err(AppError::internal)?;
        }
        let (armed, testing) = {
            let mut st = self.f5.lock();
            st.settings.wake_enabled = enabled;
            st.settings.wake_accept_risk = accept_risk;
            st.settings.wake_owner_gate = owner_gate;
            st.wake.error = None;
            (st.wake.armed, st.wake.test.active)
        };
        if enabled {
            // Nowa bramka właściciela / próg — nasłuch uzbrajany od nowa.
            if armed {
                self.ctl(Ctl::DisarmWake);
            }
            self.arm_wake()
        } else {
            if !testing {
                self.disarm_wake();
            }
            Ok(())
        }
    }

    fn wake_test(&self, on: bool) -> Result<(), AppError> {
        if !on {
            let enabled = {
                let mut st = self.f5.lock();
                st.wake.test.active = false;
                st.wake.test_until_ms = None;
                st.settings.wake_enabled
            };
            if !enabled {
                self.disarm_wake();
            }
            return Ok(());
        }
        let armed = {
            let mut st = self.f5.lock();
            st.wake.test = WakeTestView {
                active: true,
                ..WakeTestView::default()
            };
            st.wake.test_until_ms = None;
            st.wake.armed
        };
        if armed {
            return Ok(());
        }
        let result = self.arm_wake();
        if result.is_err() {
            self.f5.lock().wake.test.active = false;
        }
        result
    }

    /// Nasłuch dla bieżących ustawień (model z fabryki, próg z pomiaru, bramka właściciela).
    pub(crate) fn wake_listener(&self, owner_gate: bool) -> Result<WakeWordListener, AppError> {
        let parts = self
            .f5
            .parts()
            .ok_or_else(|| AppError::unavailable("Słowa wywoławcze", "voice-wake"))?;
        let scorer = match parts.wake_scorer() {
            Some(Ok(s)) => s,
            Some(Err(e)) => return Err(unavailable(format!("Model słów wywoławczych: {e}"))),
            None => return Err(unavailable(view::NO_KWS_MODEL)),
        };
        let threshold = parts
            .wake_calibration()
            .map(|c| c.threshold)
            .filter(|t| *t > 0.0 && *t < 1.0)
            .unwrap_or(DEFAULT_THRESHOLD);
        let mut cfg = WakeWordCfg::from_personas(&builtin_personas(), threshold);
        cfg.owner_gate = owner_gate;
        let mut listener = WakeWordListener::new(&cfg, KwsParams::default(), scorer)
            .map_err(|e| AppError::invalid(e.to_string()))?;
        if owner_gate {
            let verifier = self.f5.verifier().map_err(unavailable)?;
            listener = listener.with_owner_check(Box::new(SpeakerOwnerCheck(verifier)));
        }
        Ok(listener)
    }

    /// Uzbraja nasłuch (startuje potok, gdy trzeba).
    pub(crate) fn arm_wake(&self) -> Result<(), AppError> {
        let owner_gate = self.f5.lock().settings.wake_owner_gate;
        let listener = self.wake_listener(owner_gate)?;
        let tx = self.ensure_running()?;
        tx.send(Ctl::ArmWake(Box::new(listener)))
            .map_err(|_| AppError::internal("pętla potoku głosu zakończona"))?;
        let mut st = self.f5.lock();
        st.wake.armed = true;
        st.wake.suspended = false;
        Ok(())
    }

    /// Rozbraja nasłuch; bez trwającej rozmowy potok się wyłącza.
    pub(crate) fn disarm_wake(&self) {
        self.ctl(Ctl::DisarmWake);
        if !lock(&self.shared).conversation {
            self.ctl(Ctl::Stop);
        }
        let mut st = self.f5.lock();
        st.wake.armed = false;
        st.wake.listening = false;
    }

    /// Wstrzymanie nasłuchu na czas dyktowania / nagrania rejestracji (mikrofon ma jedno zadanie).
    pub(crate) fn suspend_wake(&self) {
        let armed = self.f5.lock().wake.armed;
        if armed {
            self.ctl(Ctl::DisarmWake);
            let mut st = self.f5.lock();
            st.wake.armed = false;
            st.wake.suspended = true;
        }
    }

    /// Powrót nasłuchu po dyktowaniu / nagraniu (jeśli był wstrzymany i nadal włączony).
    pub(crate) fn resume_wake(&self) {
        let wanted = {
            let mut st = self.f5.lock();
            let wanted = st.wake.suspended && (st.settings.wake_enabled || st.wake.test.active);
            st.wake.suspended = false;
            wanted
        };
        if wanted && let Err(e) = self.arm_wake() {
            self.f5.lock().wake.error = Some(e.message);
        }
        self.f5.publish();
    }

    /// Start aplikacji: włączone wcześniej słowa wywoławcze wracają, jeśli warunki nadal są
    /// spełnione (profil, model); inaczej zostają wyłączone z powodem w widoku.
    pub(crate) async fn wake_autostart(&self) {
        self.f5.ensure_loaded().await;
        let (s, armed) = {
            let st = self.f5.lock();
            (st.settings.clone(), st.wake.armed)
        };
        if !s.wake_enabled || armed {
            return;
        }
        let result = self
            .wake_check_enable(s.wake_accept_risk, s.wake_owner_gate)
            .and_then(|()| self.arm_wake());
        if let Err(e) = result {
            tracing::warn!(error = %e.message, "słowa wywoławcze nie wystartowały");
            self.f5.lock().wake.error = Some(e.message);
        }
        self.f5.publish();
    }
}
