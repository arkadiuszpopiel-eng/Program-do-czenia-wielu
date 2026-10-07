//! Głos rozszerzony F5 w aplikacji: słowa wywoławcze z bramką właściciela ([`wake`]), kreator
//! rejestracji i weryfikacja właściciela ([`speaker`]), dyktowanie do dowolnej aplikacji
//! ([`dictation`]), czytanie zaznaczenia/dokumentu/schowka ([`read`]); stan dla UI ([`view`]) i
//! ustawienia w `core-config` ([`settings`]).
//!
//! Zasady wspólne: wszystko domyślnie wyłączone i włączane jawnie; audio, embedding mówcy,
//! dyktowany i czytany tekst nie trafiają do zdarzeń, logów ani pamięci; w trakcie dyktowania
//! i czytania tury głosowe nie trafiają do modelu (fail-closed — [`F5::voice_busy`]).

pub(crate) mod dictation;
pub(crate) mod read;
pub(crate) mod settings;
pub(crate) mod speaker;
pub(crate) mod view;
pub(crate) mod wake;

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use app_api::dto::{AlfaEvent, SpeakerCheckView, VoiceFeatures};
use app_api::events::EventHub;
use core_config_contract::ConfigStore;
use platform_contract::{ClipboardPort, DesktopPort, InputPort, UiaPort};
use scheduler_lite_contract::SchedulerLite;
use voice_audio_contract::{AudioIo, OutputStream};
use voice_dictation_impl::DictationAudio;
use voice_speaker_contract::{SpeakerError, SpeakerVerifier};
use voice_tts_contract::Tts;
use voice_wake_contract::{KeywordScorer, WakeError};

use crate::engine::{Pacer, VoiceEngineFactory};

pub use settings::Settings;

/// Pomiar FAR/FRR słów wywoławczych z runnera `alfa-wake-eval run` (ACCEPTANCE F5-05/06).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WakeCalibration {
    /// Fałszywe wybudzenia na dobę przy progu oceny.
    pub far_per_day: f64,
    /// Odsetek nierozpoznanych pozytywów (0–1).
    pub frr: f64,
    /// Liczności wystarczają (≥ 24 h tła, ≥ 200 pozytywów).
    pub sufficient: bool,
    /// FAR ≤ 1/dzień i FRR ≤ 5 %.
    pub passes: bool,
    /// Próg oceny (używany przez detektor).
    pub threshold: f32,
}

impl WakeCalibration {
    /// Wynik runnera (`voice_wake_impl::eval::Summary`).
    pub fn from_summary(s: &voice_wake_impl::eval::Summary) -> Self {
        Self {
            far_per_day: s.at_threshold.far_per_day(),
            frr: s.at_threshold.frr(),
            sufficient: s.sufficient,
            passes: s.f5_05_far_ok && s.f5_06_frr_ok,
            threshold: s.threshold,
        }
    }

    /// Pomiar na pełnym korpusie spełnia progi — włączenie bez potwierdzania ryzyka.
    pub fn trusted(&self) -> bool {
        self.sufficient && self.passes
    }
}

/// Składniki głosu rozszerzonego (produkcja: modele i sidecary z `AppPaths`; testy: atrapy na
/// wirtualnym zegarze). Zwraca je fabryka potoku ([`VoiceEngineFactory::features`]).
pub trait FeatureFactory: Send + Sync {
    /// Czy jest model słów wywoławczych (bez ładowania).
    fn has_wake_model(&self) -> bool;
    /// Model słów wywoławczych (`None` — brak modelu).
    fn wake_scorer(&self) -> Option<Result<Box<dyn KeywordScorer>, WakeError>>;
    /// Pomiar FAR/FRR z runnera (`None` — brak pomiaru: „nieskalibrowane”).
    fn wake_calibration(&self) -> Option<WakeCalibration>;
    /// Weryfikator mówcy (`None` — brak modelu). Wywoływane raz, wynik współdzielony.
    fn speaker(&self) -> Option<Result<Arc<dyn SpeakerVerifier>, SpeakerError>>;
    /// Wejście audio (nagrania kreatora rejestracji).
    fn audio(&self) -> Option<Arc<dyn AudioIo>>;
    /// Mikrofon, VAD, STT i scheduler dyktowania.
    fn dictation_audio(&self) -> Result<DictationAudio, String>;
    /// Synteza i wyjście audio czytania.
    fn read_audio(&self) -> Result<ReadAudio, String>;
    /// Zasoby wyłączne (mikrofon, głośnik).
    fn scheduler(&self) -> Arc<dyn SchedulerLite>;
    /// Rytm pętli dyktowania, czytania i nagrywania.
    fn pacer(&self, period: Duration) -> Box<dyn Pacer>;
    /// Zegar pętli (ms).
    fn now_ms(&self) -> u64;
}

/// Synteza i wyjście audio czytania na głos.
pub struct ReadAudio {
    /// Synteza (głos agentki; treść czytana — `PrivacyTag::Private`).
    pub tts: Arc<dyn Tts>,
    /// Wyjście audio (mikser).
    pub output: Box<dyn OutputStream>,
}

/// Porty pulpitu dyktowania i czytania (te same co computer use: strażnik okien Alfy/Brokera).
#[derive(Clone)]
pub struct DesktopDeps {
    /// Okna (pierwszy plan, strażnik celów).
    pub desktop: Arc<dyn DesktopPort>,
    /// UI Automation (pola haseł, tekst).
    pub uia: Arc<dyn UiaPort>,
    /// Wejście syntetyczne (dyktowanie; Ctrl+C zapasu czytania).
    pub input: Arc<dyn InputPort>,
    /// Schowek (czytanie schowka, zapas Ctrl+C).
    pub clipboard: Option<Arc<dyn ClipboardPort>>,
}

/// Zależności głosu rozszerzonego z kompozycji aplikacji.
#[derive(Clone, Default)]
pub struct FeatureDeps {
    /// Ustawienia (`voice.*`); `None` — tylko w pamięci procesu.
    pub config: Option<Arc<dyn ConfigStore>>,
    /// Porty pulpitu; `None` — dyktowanie i czytanie okien niedostępne.
    pub desktop: Option<DesktopDeps>,
}

/// Stan współdzielony głosu rozszerzonego (port, pętla potoku, źródło odpowiedzi, zadania).
pub(crate) struct F5 {
    pub factory: Option<Arc<dyn VoiceEngineFactory>>,
    pub deps: FeatureDeps,
    pub events: EventHub,
    pub st: Mutex<F5State>,
    speaker: OnceLock<Result<Arc<dyn SpeakerVerifier>, String>>,
    last: Mutex<Option<VoiceFeatures>>,
}

/// Stan bieżący.
#[derive(Default)]
pub(crate) struct F5State {
    pub settings: Settings,
    pub loaded: bool,
    pub agent: String,
    /// Jest model słów wywoławczych (odświeżane przy otwarciu widoku — bez I/O w każdym kroku).
    pub kws_model: bool,
    /// Pomiar FAR/FRR (jw.).
    pub calibration: Option<WakeCalibration>,
    pub wake: wake::WakeRt,
    pub enroll: speaker::EnrollRt,
    pub last_check: Option<SpeakerCheckView>,
    pub dictation: dictation::DictRt,
    pub read: read::ReadRt,
}

impl F5 {
    pub(crate) fn new(
        factory: Option<Arc<dyn VoiceEngineFactory>>,
        deps: FeatureDeps,
        events: EventHub,
    ) -> Self {
        Self {
            factory,
            deps,
            events,
            st: Mutex::new(F5State {
                agent: "alfa".into(),
                ..F5State::default()
            }),
            speaker: OnceLock::new(),
            last: Mutex::new(None),
        }
    }

    /// Blokada stanu (zatruta — dalej używalna: stan jest prosty i spójny po każdej operacji).
    pub(crate) fn lock(&self) -> MutexGuard<'_, F5State> {
        self.st.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Składniki F5 z fabryki potoku.
    pub(crate) fn parts(&self) -> Option<&dyn FeatureFactory> {
        self.factory.as_deref().and_then(|f| f.features())
    }

    /// Weryfikator mówcy (model ładowany przy pierwszym użyciu); błąd — tekst dla UI.
    pub(crate) fn verifier(&self) -> Result<Arc<dyn SpeakerVerifier>, String> {
        let Some(parts) = self.parts() else {
            return Err(view::NO_SPEAKER_MODEL.into());
        };
        self.speaker
            .get_or_init(|| match parts.speaker() {
                Some(Ok(v)) => Ok(v),
                Some(Err(e)) => Err(e.to_string()),
                None => Err(view::NO_SPEAKER_MODEL.into()),
            })
            .clone()
    }

    /// Odświeża dostępność modelu słów wywoławczych i pomiar FAR/FRR (pliki modeli).
    pub(crate) fn refresh(&self) {
        let (kws, cal) = self.parts().map_or((false, None), |p| {
            (p.has_wake_model(), p.wake_calibration())
        });
        let mut st = self.lock();
        st.kws_model = kws;
        st.calibration = cal;
    }

    /// Wczytuje ustawienia z konfiguracji (raz).
    pub(crate) async fn ensure_loaded(&self) {
        if self.lock().loaded {
            return;
        }
        self.refresh();
        let loaded = settings::load(self.deps.config.as_deref()).await;
        let mut st = self.lock();
        if !st.loaded {
            st.settings = loaded;
            st.loaded = true;
        }
    }

    /// Trwa dyktowanie albo czytanie — tury głosowe nie idą do modelu.
    pub(crate) fn voice_busy(&self) -> bool {
        let st = self.lock();
        st.dictation.active() || st.read.active()
    }

    /// Wynik weryfikacji ostatniej tury (widok).
    pub(crate) fn note_check(&self, check: SpeakerCheckView) {
        self.lock().last_check = Some(check);
        self.publish();
    }

    /// Widok bieżący.
    pub(crate) fn view(&self) -> VoiceFeatures {
        view::build(self)
    }

    /// Wysyła widok do UI, gdy się zmienił.
    pub(crate) fn publish(&self) {
        let now = self.view();
        let mut last = self.last.lock().unwrap_or_else(|p| p.into_inner());
        if last.as_ref() != Some(&now) {
            *last = Some(now.clone());
            self.events.emit(AlfaEvent::VoiceFeaturesChanged {
                features: Box::new(now),
            });
        }
    }
}
