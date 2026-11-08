//! Stan usługi → DTO UI i błędy modułu → błędy komend (komunikaty po polsku, bez ścieżek).

use app_api::AppError;
use app_api::dto::{
    LocalizedText, UpdateChannel, UpdateMode as ModeDto, UpdatePhase as PhaseDto, UpdateProgress,
    UpdateRelease, UpdatesView, iso,
};
use updater_contract::{Channel, UpdateMode, UpdatePhase, UpdateStatus, UpdaterError};

/// Kanał → DTO.
pub(crate) fn channel_dto(channel: Channel) -> UpdateChannel {
    match channel {
        Channel::Stable => UpdateChannel::Stable,
        Channel::Beta => UpdateChannel::Beta,
    }
}

fn phase_dto(phase: UpdatePhase) -> PhaseDto {
    match phase {
        UpdatePhase::Disabled => PhaseDto::Disabled,
        UpdatePhase::Idle => PhaseDto::Idle,
        UpdatePhase::Checking => PhaseDto::Checking,
        UpdatePhase::UpToDate => PhaseDto::UpToDate,
        UpdatePhase::Available => PhaseDto::Available,
        UpdatePhase::Downloading => PhaseDto::Downloading,
        UpdatePhase::Verifying => PhaseDto::Verifying,
        UpdatePhase::Installing => PhaseDto::Installing,
        UpdatePhase::Ready => PhaseDto::Ready,
        UpdatePhase::Failed => PhaseDto::Failed,
    }
}

/// Stan usługi → widok dla UI; `blocked` — powód, dla którego restart teraz nie ruszy.
pub fn view_of(s: &UpdateStatus, blocked: Option<LocalizedText>) -> UpdatesView {
    UpdatesView {
        phase: phase_dto(s.phase),
        current: s.current.to_string(),
        channel: channel_dto(s.channel),
        mode: match s.mode {
            UpdateMode::Auto => ModeDto::Auto,
            UpdateMode::Ask => ModeDto::Ask,
            UpdateMode::Manual => ModeDto::Manual,
        },
        available: s.available.as_ref().map(|a| UpdateRelease {
            version: a.version.to_string(),
            notes: a.notes.clone(),
        }),
        progress: s.progress.map(|p| UpdateProgress {
            downloaded: p.downloaded,
            total: p.total,
            resumed: p.resumed,
        }),
        ready: s.ready.as_ref().map(ToString::to_string),
        previous: s.previous.as_ref().map(ToString::to_string),
        last_check: s.last_check.map(iso),
        error: s.error.clone(),
        restart_blocked: blocked,
    }
}

/// Błąd modułu → błąd komendy.
pub fn error_of(e: &UpdaterError) -> AppError {
    match e {
        UpdaterError::NotConfigured { .. } | UpdaterError::NoPublicKey => AppError::unavailable(
            "Aktualizacje (adres wydań i klucz minisign ustawia wydanie)",
            "updater",
        ),
        UpdaterError::NoPrevious => AppError::not_found("Brak poprzedniej wersji do przywrócenia."),
        UpdaterError::Downgrade { version, current } => AppError::forbidden(format!(
            "Wersja {version} nie jest nowsza od {current} — tylko jawny powrót do starszej wersji."
        )),
        UpdaterError::SignatureInvalid { .. } | UpdaterError::HashMismatch => {
            AppError::forbidden(format!("Paczka aktualizacji odrzucona: {e}"))
        }
        UpdaterError::UnsafePackage { .. } => {
            AppError::forbidden(format!("Paczka aktualizacji odrzucona: {e}"))
        }
        UpdaterError::Io { .. } => AppError::storage(e),
        UpdaterError::Network { .. }
        | UpdaterError::Cancelled
        | UpdaterError::NoUsableVersion { .. }
        | UpdaterError::NotInstalled { .. }
        | UpdaterError::Invalid { .. } => AppError::invalid(e.to_string()),
    }
}
