//! Dyktowanie do dowolnej aplikacji (`voice-dictation`): skrót globalny / przycisk → cel = okno
//! na pierwszym planie, `DictationRunner` (mikrofon z dzierżawą, VAD, STT) → `DictationService`
//! (wpisywanie przez `InputPort` w porcjach, strażnik celów, **odmowa w polach haseł**, fail-closed).
//! Profile per aplikacja (wielka litera na początku, Enter w terminalach). Podgląd = ostatnia
//! fraza, tylko w widoku okna Alfy; dyktowany tekst nie trafia na magistralę, do logów ani pamięci.
//! Na czas dyktowania rozmowa głosowa i nasłuch słów wywoławczych są wstrzymane (mikrofon ma
//! jedno zadanie; tury głosowe nie idą do modelu — [`super::F5::voice_busy`]).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_api::dto::{DictationAction, DictationProfile, LocalizedText, VoiceFeatures};
use app_api::error::{AppError, ErrorCode};
use platform_contract::image_file_name;
use tokio::sync::mpsc;
use voice_dictation_contract::{
    Dictation, DictationCfg, DictationError, DictationEvent, DictationMode, DictationPhase,
    DictationStatus,
};
use voice_dictation_impl::{DictationPorts, DictationRunner, DictationService};

use super::settings::{self, DICTATION_PROFILES, MAX_PROFILES, normalize_profile};
use super::{DesktopDeps, F5, view};
use crate::engine::Pacer;
use crate::port::{Voice, lock};

/// Skrót globalny dyktowania (rejestruje powłoka; reguła AltGr: D nie jest literą polską).
pub const SHORTCUT: &str = "Ctrl+Alt+D";
/// Rytm pętli dyktowania.
const TICK: Duration = Duration::from_millis(20);
/// Ile razy ponowić start, gdy mikrofon zwalnia jeszcze rozmowa głosowa.
const BUSY_RETRIES: u32 = 50;

/// Sterowanie trwającą sesją.
pub(crate) enum DictCmd {
    Stop,
    Undo,
}

/// Stan dyktowania.
#[derive(Default)]
pub(crate) struct DictRt {
    pub ctl: Option<mpsc::UnboundedSender<DictCmd>>,
    pub starting: bool,
    pub status: Option<DictationStatus>,
    pub preview: Option<String>,
    pub refused: Option<LocalizedText>,
}

impl DictRt {
    /// Sesja trwa (albo właśnie startuje).
    pub fn active(&self) -> bool {
        self.starting || self.ctl.as_ref().is_some_and(|c| !c.is_closed())
    }
}

/// Usługa z podglądem ostatniej frazy (tylko pamięć procesu, czyszczona po sesji).
struct Preview<D: Dictation> {
    inner: D,
    last: Arc<Mutex<Option<String>>>,
}

impl<D: Dictation> Dictation for Preview<D> {
    fn start(
        &mut self,
        mode: DictationMode,
        now_ms: u64,
    ) -> Result<DictationStatus, DictationError> {
        self.inner.start(mode, now_ms)
    }
    fn stop(&mut self) -> DictationStatus {
        self.inner.stop()
    }
    fn on_final(&mut self, text: &str, now_ms: u64) -> Result<DictationStatus, DictationError> {
        if let Ok(mut last) = self.last.lock() {
            *last = Some(text.trim().to_owned());
        }
        self.inner.on_final(text, now_ms)
    }
    fn tick(&mut self, now_ms: u64) -> DictationStatus {
        self.inner.tick(now_ms)
    }
    fn undo_last(&mut self, now_ms: u64) -> Result<DictationStatus, DictationError> {
        self.inner.undo_last(now_ms)
    }
    fn status(&self) -> DictationStatus {
        self.inner.status()
    }
    fn take_events(&mut self) -> Vec<DictationEvent> {
        self.inner.take_events()
    }
}

type Runner = DictationRunner<Preview<DictationService>>;

impl F5 {
    fn dictation_cmd(&self, cmd: DictCmd) -> bool {
        self.lock()
            .dictation
            .ctl
            .as_ref()
            .is_some_and(|c| c.send(cmd).is_ok())
    }

    /// Koniec dyktowania (kill-switch, „stop wszystko”).
    pub(crate) fn stop_dictation(&self) {
        self.dictation_cmd(DictCmd::Stop);
    }

    fn now_ms(&self) -> u64 {
        self.parts().map_or(0, |p| p.now_ms())
    }
}

fn unavailable(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::Unavailable, message)
}

impl Voice {
    /// `voice_dictation`: start/stop/przełącz, „cofnij”, profile aplikacji.
    pub(crate) async fn dictation_action(
        self: &Arc<Self>,
        action: DictationAction,
    ) -> Result<VoiceFeatures, AppError> {
        self.f5.ensure_loaded().await;
        let active = self.f5.lock().dictation.active();
        let result = match action {
            DictationAction::Start if active => Ok(()),
            DictationAction::Start => self.dictation_start().await,
            DictationAction::Toggle if active => {
                self.f5.dictation_cmd(DictCmd::Stop);
                Ok(())
            }
            DictationAction::Toggle => self.dictation_start().await,
            DictationAction::Stop => {
                self.f5.dictation_cmd(DictCmd::Stop);
                Ok(())
            }
            DictationAction::Undo if self.f5.dictation_cmd(DictCmd::Undo) => Ok(()),
            DictationAction::Undo => Err(AppError::invalid("Dyktowanie nie trwa.")),
            DictationAction::SaveProfile { profile } => {
                self.save_profile(Some(profile), None).await
            }
            DictationAction::RemoveProfile { app } => self.save_profile(None, Some(app)).await,
        };
        self.f5.publish();
        result.map(|()| self.f5.view())
    }

    async fn save_profile(
        &self,
        add: Option<DictationProfile>,
        remove: Option<String>,
    ) -> Result<(), AppError> {
        let mut list = self.f5.lock().settings.profiles.clone();
        if let Some(app) = remove {
            let app = image_file_name(&app).to_lowercase();
            list.retain(|p| p.app != app);
        }
        if let Some(p) = add {
            let p = normalize_profile(p).map_err(AppError::invalid)?;
            list.retain(|x| x.app != p.app);
            if list.len() >= MAX_PROFILES {
                return Err(AppError::invalid(format!(
                    "Najwyżej {MAX_PROFILES} profili dyktowania."
                )));
            }
            list.push(p);
            list.sort_by(|a, b| a.app.cmp(&b.app));
        }
        let value = serde_json::to_value(&list).map_err(AppError::internal)?;
        settings::save(self.f5.deps.config.as_deref(), DICTATION_PROFILES, value)
            .await
            .map_err(AppError::internal)?;
        self.f5.lock().settings.profiles = list;
        Ok(())
    }

    /// Profil aplikacji na pierwszym planie (bez profilu — ustawienia domyślne).
    fn dictation_cfg(&self, desk: &DesktopDeps) -> DictationCfg {
        let app = desk
            .desktop
            .foreground()
            .ok()
            .flatten()
            .map(|w| image_file_name(&w.image).to_lowercase());
        let mut cfg = DictationCfg::default();
        let st = self.f5.lock();
        if let Some(p) = app.and_then(|a| st.settings.profiles.iter().find(|p| p.app == a)) {
            cfg.capitalize_start = p.capitalize_start;
            cfg.block_enter_in_terminals = p.block_enter;
        }
        cfg
    }

    fn fail_start(&self, refused: Option<LocalizedText>) {
        {
            let mut st = self.f5.lock();
            st.dictation.starting = false;
            st.dictation.refused = refused;
        }
        self.resume_wake();
    }

    async fn dictation_start(self: &Arc<Self>) -> Result<(), AppError> {
        let desk = self
            .f5
            .deps
            .desktop
            .clone()
            .ok_or_else(|| AppError::unavailable("Dyktowanie", "platform-windows-gui"))?;
        let parts = self
            .f5
            .parts()
            .ok_or_else(|| AppError::unavailable("Dyktowanie", "voice-dictation"))?;
        let audio = parts
            .dictation_audio()
            .map_err(|e| unavailable(format!("Dyktowanie: {e}")))?;
        let pacer = parts.pacer(TICK);
        {
            let mut st = self.f5.lock();
            st.dictation.starting = true;
            st.dictation.refused = None;
            st.dictation.preview = None;
        }
        let cfg = self.dictation_cfg(&desk);
        let conversation = lock(&self.shared).conversation;
        if conversation {
            self.end_conversation();
        }
        self.suspend_wake();
        let ports = DictationPorts {
            desktop: desk.desktop.clone(),
            uia: desk.uia.clone(),
            input: desk.input.clone(),
        };
        let last = Arc::new(Mutex::new(None));
        let service = Preview {
            inner: DictationService::new(ports, cfg),
            last: last.clone(),
        };
        let mut runner = DictationRunner::new(audio, service);
        let mut tries = 0;
        let begin = loop {
            let r = runner.begin(DictationMode::Toggle, self.f5.now_ms()).await;
            let busy =
                matches!(&r, Err(DictationError::Platform(m)) if m.starts_with("mikrofon zajęty"));
            if busy && conversation && tries < BUSY_RETRIES {
                tries += 1;
                tokio::time::sleep(Duration::from_millis(20)).await;
                continue;
            }
            break r;
        };
        let events = runner.dictation().take_events();
        self.publish_bus(events.iter().map(DictationEvent::to_bus_event).collect())
            .await;
        let status = match begin {
            Ok(s) => s,
            Err(DictationError::Refused(reason)) => {
                let text = view::dictation_refusal(reason);
                self.fail_start(Some(text.clone()));
                return Err(AppError::forbidden(text.pl));
            }
            Err(e) => {
                let text =
                    LocalizedText::new(format!("Dyktowanie: {e}"), "Dictation could not start.");
                self.fail_start(Some(text.clone()));
                return Err(unavailable(text.pl));
            }
        };
        let (tx, rx) = mpsc::unbounded_channel();
        {
            let mut st = self.f5.lock();
            st.dictation.ctl = Some(tx);
            st.dictation.starting = false;
            st.dictation.status = Some(status);
        }
        let me = self.clone();
        tokio::spawn(async move { me.dictation_loop(runner, rx, pacer, last).await });
        Ok(())
    }

    async fn dictation_loop(
        self: Arc<Self>,
        mut runner: Runner,
        mut rx: mpsc::UnboundedReceiver<DictCmd>,
        mut pacer: Box<dyn Pacer>,
        last: Arc<Mutex<Option<String>>>,
    ) {
        loop {
            pacer.tick().await;
            let now = self.f5.now_ms();
            let mut stop = false;
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    DictCmd::Stop => stop = true,
                    DictCmd::Undo => {
                        let _ = runner.dictation().undo_last(now);
                    }
                }
            }
            let result = if stop {
                runner.end(now).await
            } else {
                runner.step(now).await
            };
            let events = runner.dictation().take_events();
            let refusal = events.iter().find_map(|e| match e {
                DictationEvent::Refused { reason } => Some(view::dictation_refusal(*reason)),
                _ => None,
            });
            self.publish_bus(events.iter().map(DictationEvent::to_bus_event).collect())
                .await;
            let status = match result {
                Ok(s) => s,
                Err(e) => {
                    let _ = runner.end(now).await;
                    let mut st = self.f5.lock();
                    st.dictation.refused = Some(refusal.unwrap_or_else(|| {
                        LocalizedText::new(format!("Dyktowanie: {e}"), "Dictation stopped.")
                    }));
                    st.dictation.status = Some(runner.dictation().status());
                    break;
                }
            };
            let idle = status.phase == DictationPhase::Idle;
            {
                let mut st = self.f5.lock();
                if refusal.is_some() {
                    st.dictation.refused = refusal;
                }
                st.dictation.status = Some(status);
                st.dictation.preview = last.lock().ok().and_then(|l| l.clone());
            }
            self.f5.publish();
            if idle || stop {
                break;
            }
        }
        drop(runner);
        {
            let mut st = self.f5.lock();
            st.dictation.ctl = None;
            st.dictation.preview = None;
        }
        if let Ok(mut l) = last.lock() {
            *l = None;
        }
        self.resume_wake();
        self.f5.publish();
    }
}
