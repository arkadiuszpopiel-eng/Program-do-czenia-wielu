//! Silniki TTS z sidecarów (`voice-tts-impl`): Pocket TTS PL (trwały proces, JSON-lines) i Piper
//! `pl_PL` (proces na zdanie), łańcuchy głosów v0 bez kluczy. Brak obu plików = brak silnika
//! (czytanie na głos zwraca „brak silnika TTS — pobierz w Ustawieniach → Głos").

use std::sync::Arc;

use voice_tts_contract::{Tts, v0_chains};
use voice_tts_impl::TtsService;
use voice_tts_impl::engines::{PiperEngine, PocketSidecar, TokioRunner, TtsBackend};

use app_api::AppPaths;
use app_api::error::AppError;

/// Serwis TTS z zainstalowanych sidecarów (`None` — żaden nie jest zainstalowany).
pub fn engines(paths: &AppPaths) -> Result<Option<Arc<dyn Tts>>, AppError> {
    let mut backends: Vec<Arc<dyn TtsBackend>> = Vec::new();
    let pocket = paths.sidecar("pocket-tts", "pocket-tts");
    if pocket.is_file() {
        let models = paths.models().join("pocket-tts");
        let args = vec!["--models".to_owned(), models.to_string_lossy().into_owned()];
        backends.push(Arc::new(PocketSidecar::process(pocket, args)));
    }
    let piper = paths.sidecar("piper", "piper");
    if piper.is_file() {
        backends.push(Arc::new(PiperEngine::new(
            piper,
            paths.models().join("piper"),
            Arc::new(TokioRunner),
        )));
    }
    if backends.is_empty() {
        return Ok(None);
    }
    let service = TtsService::new(v0_chains(), backends)
        .map_err(|e| AppError::internal(format!("voice-tts: {e}")))?;
    Ok(Some(Arc::new(service)))
}
