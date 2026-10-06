//! Silniki TTS z sidecarów (`voice-tts-impl`): Pocket TTS PL (trwały proces, JSON-lines) i Piper
//! `pl_PL` (proces na zdanie), łańcuchy głosów v0 bez kluczy. Silniki są wykrywane **przy użyciu**
//! ([`InstalledTts`]), tak jak `llama-server` (`route::local::candidates`): Piper pobrany
//! w Ustawieniach działa od następnego czytania na głos albo włączenia rozmowy głosowej — bez
//! ponownego uruchomienia Alfy. Brak obu plików = stan `Failed` ([`ready`] → `None`): czytanie na
//! głos zwraca „brak silnika TTS — pobierz w Ustawieniach → Głos", rozmowa — „głos niedostępny".

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use personas_contract::PersonaId;
use voice_tts_contract::{
    CancelToken, Tts, TtsError, TtsEvent, TtsHealth, TtsRequest, TtsStream, VoiceInfo, v0_chains,
};
use voice_tts_impl::TtsService;
use voice_tts_impl::engines::{PiperEngine, PocketSidecar, TokioRunner, TtsBackend};

use app_api::AppPaths;

use crate::voice::NO_TTS;

/// Zainstalowane silniki (pliki sidecarów w chwili sprawdzenia).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Installed {
    pocket: bool,
    piper: bool,
}

impl Installed {
    fn detect(paths: &AppPaths) -> Self {
        Self {
            pocket: paths.sidecar("pocket-tts", "pocket-tts").is_file(),
            piper: paths.sidecar("piper", "piper").is_file(),
        }
    }
}

/// Serwis TTS dla zainstalowanych silników (`None` — żaden nie jest zainstalowany).
fn build(paths: &AppPaths, installed: Installed) -> Result<Option<Arc<dyn Tts>>, TtsError> {
    let mut backends: Vec<Arc<dyn TtsBackend>> = Vec::new();
    if installed.pocket {
        let pocket = paths.sidecar("pocket-tts", "pocket-tts");
        let models = paths.models().join("pocket-tts");
        let args = vec!["--models".to_owned(), models.to_string_lossy().into_owned()];
        backends.push(Arc::new(PocketSidecar::process(pocket, args)));
    }
    if installed.piper {
        backends.push(Arc::new(PiperEngine::new(
            paths.sidecar("piper", "piper"),
            paths.models().join("piper"),
            Arc::new(TokioRunner),
        )));
    }
    if backends.is_empty() {
        return Ok(None);
    }
    Ok(Some(Arc::new(TtsService::new(v0_chains(), backends)?)))
}

/// TTS z silników zainstalowanych **w chwili użycia**: każde wywołanie sprawdza pliki sidecarów
/// i — gdy zestaw się zmienił (pobranie w Ustawieniach) — składa serwis od nowa.
pub struct InstalledTts {
    paths: AppPaths,
    current: Mutex<Option<(Installed, Arc<dyn Tts>)>>,
}

impl InstalledTts {
    /// TTS nad katalogami aplikacji.
    pub fn new(paths: AppPaths) -> Self {
        Self {
            paths,
            current: Mutex::new(None),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<(Installed, Arc<dyn Tts>)>> {
        self.current.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Serwis dla silników zainstalowanych teraz.
    fn service(&self) -> Result<Arc<dyn Tts>, TtsError> {
        let now = Installed::detect(&self.paths);
        let mut current = self.lock();
        if let Some((installed, service)) = current.as_ref()
            && *installed == now
        {
            return Ok(Arc::clone(service));
        }
        let service = build(&self.paths, now)?;
        *current = service.clone().map(|s| (now, s));
        if service.is_some() {
            tracing::info!(
                pocket = now.pocket,
                piper = now.piper,
                "silniki TTS wykryte"
            );
        }
        service.ok_or_else(|| TtsError::NotAvailable(NO_TTS.into()))
    }

    /// Ostatni serwis (bez ponownego wykrywania): zatrzymanie i zdarzenia trafiają tam, gdzie
    /// trwa wypowiedź.
    fn cached(&self) -> Option<Arc<dyn Tts>> {
        self.lock().as_ref().map(|(_, s)| Arc::clone(s))
    }
}

#[async_trait]
impl Tts for InstalledTts {
    fn voices(&self) -> Vec<VoiceInfo> {
        self.service().map(|s| s.voices()).unwrap_or_default()
    }

    async fn synth(&self, request: TtsRequest, cancel: CancelToken) -> Result<TtsStream, TtsError> {
        self.service()?.synth(request, cancel).await
    }

    fn stop(&self, utterance: u64) {
        if let Some(s) = self.cached() {
            s.stop(utterance);
        }
    }

    async fn warm(&self, persona: &PersonaId) -> Result<(), TtsError> {
        self.service()?.warm(persona).await
    }

    fn health(&self) -> TtsHealth {
        match self.service() {
            Ok(s) => s.health(),
            Err(e) => TtsHealth::Failed(e.to_string()),
        }
    }

    fn take_events(&self) -> Vec<TtsEvent> {
        self.cached().map(|s| s.take_events()).unwrap_or_default()
    }
}

/// TTS aplikacji: silniki z sidecarów wykrywane przy każdym użyciu.
pub fn engines(paths: &AppPaths) -> Arc<dyn Tts> {
    Arc::new(InstalledTts::new(paths.clone()))
}

/// TTS gotowy teraz (`None` — brak TTS albo żaden silnik nie jest zainstalowany: `Failed`).
pub fn ready(tts: Option<&Arc<dyn Tts>>) -> Option<Arc<dyn Tts>> {
    tts.filter(|t| !matches!(t.health(), TtsHealth::Failed(_)))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fala 6: Piper pobrany po starcie aplikacji jest wykrywany przy następnym użyciu.
    #[tokio::test]
    async fn piper_installed_after_start_is_used_without_restart() {
        let dir = std::env::temp_dir().join(format!("alfa-tts-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths::under(&dir);
        let tts = engines(&paths);
        assert!(matches!(tts.health(), TtsHealth::Failed(m) if m.contains(NO_TTS)));
        assert!(ready(Some(&tts)).is_none() && ready(None).is_none());
        assert!(tts.voices().is_empty() && tts.take_events().is_empty());
        let request = TtsRequest {
            utterance: 1,
            persona: PersonaId::alfa(),
            text: "Dzień dobry.".into(),
            style: voice_tts_contract::SpeechStyle::default(),
            cacheable: false,
            privacy: providers_contract::PrivacyTag::Normal,
        };
        let err = tts.synth(request.clone(), CancelToken::new()).await;
        assert!(matches!(err, Err(TtsError::NotAvailable(_))));
        tts.stop(1);

        let piper = paths.sidecar("piper", "piper");
        std::fs::create_dir_all(piper.parent().unwrap()).unwrap();
        std::fs::write(&piper, b"exe").unwrap();
        assert_eq!(tts.health(), TtsHealth::Ready, "bez ponownego uruchomienia");
        assert!(ready(Some(&tts)).is_some());
        assert_eq!(tts.voices().len(), v0_chains().len());
        let same = tts.voices();
        assert_eq!(
            tts.voices(),
            same,
            "serwis składany raz dla tego samego zestawu"
        );

        std::fs::remove_file(&piper).unwrap();
        assert!(
            matches!(tts.health(), TtsHealth::Failed(_)),
            "silnik usunięty"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
