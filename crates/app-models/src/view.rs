//! Widok pozycji dla UI: stan wykrywany tanio (bez hashowania) z zadań w toku, ostatniego błędu,
//! rekordu instalacji, plików oczekujących na zgodę, plików częściowych i obecności plików.

use app_api::dto::{ModelFileView, ModelItem, ModelItemState, ModelProgressView};

use crate::catalog::{Install, ItemSpec};
use crate::store::{Hashes, Store};

/// Faza zadania w toku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Czeka na wolne miejsce (limit równoległości).
    Queued,
    /// Pobiera.
    Downloading,
    /// Sprawdza hashe i instaluje (rozpakowanie, manifest).
    Installing,
}

/// Stan w pamięci: zadanie w toku (faza, postęp) i ostatni błąd.
#[derive(Debug, Clone, Default)]
pub struct Live {
    /// Faza zadania (brak — nic się nie dzieje).
    pub phase: Option<Phase>,
    /// Postęp pobierania.
    pub progress: Option<ModelProgressView>,
    /// Ostatni błąd.
    pub error: Option<String>,
}

fn state_of(spec: &ItemSpec, store: &Store, live: &Live) -> (ModelItemState, Option<Hashes>) {
    match live.phase {
        Some(Phase::Queued) => return (ModelItemState::Queued, None),
        Some(Phase::Downloading) => return (ModelItemState::Downloading, None),
        Some(Phase::Installing) => return (ModelItemState::Installing, None),
        None => {}
    }
    if live.error.is_some() {
        return (ModelItemState::Failed, None);
    }
    let target = spec.target(store.paths());
    if let Some(receipt) = store.receipt(&spec.id) {
        // Model GGUF jest „zainstalowany" dopiero z rekordem `<plik>.sha256` — tak ocenia go dostawca
        // lokalny. Bez tej zgodności interfejs pokazywał „Zainstalowano", a rozmowa kończyła się
        // błędem „model nie jest pobrany".
        let recorded = |rel: &String| {
            !matches!(spec.install, Install::Gguf) || gguf_recorded(spec, &target, rel)
        };
        let ok = receipt.corrupt.is_none()
            && receipt
                .files
                .keys()
                .all(|rel| target.join(rel).is_file() && recorded(rel));
        let state = if ok {
            ModelItemState::Installed
        } else {
            ModelItemState::Corrupt
        };
        return (state, Some(receipt.downloads));
    }
    if let Some(pending) = store.pending(&spec.id) {
        return (ModelItemState::NeedsTrust, Some(pending.hashes));
    }
    if store.partial(spec).is_some() {
        return (ModelItemState::Paused, None);
    }
    if spec.present(&target) {
        let llm_recorded = matches!(spec.install, Install::Gguf)
            && spec
                .files
                .iter()
                .all(|f| gguf_recorded(spec, &target, &f.name));
        let state = if llm_recorded {
            // Pobrany przez `providers-local` (onboarding) — hash zapisany obok pliku.
            ModelItemState::Installed
        } else {
            ModelItemState::External
        };
        return (state, None);
    }
    (ModelItemState::Missing, None)
}

/// Model GGUF z rekordem `<plik>.sha256` (64 znaki) zgodnym z przypiętą sumą, jeśli ją ma —
/// to samo kryterium co `providers_local_impl::installed` (inaczej menedżer pokazywał
/// „Zainstalowano”, a rozmowa — „model nie jest pobrany”; przegląd 21.10).
fn gguf_recorded(spec: &ItemSpec, target: &std::path::Path, rel: &str) -> bool {
    let Ok(recorded) = std::fs::read_to_string(providers_local_impl::hash_path(&target.join(rel)))
    else {
        return false;
    };
    let recorded = recorded.trim();
    let pinned = spec
        .files
        .iter()
        .find(|f| f.name == rel)
        .and_then(|f| f.sha256.as_deref());
    recorded.len() == 64 && pinned.is_none_or(|p| p.eq_ignore_ascii_case(recorded))
}

/// Widok pozycji.
pub fn item(spec: &ItemSpec, store: &Store, live: &Live, active: bool) -> ModelItem {
    let (state, hashes) = state_of(spec, store, live);
    let files = spec
        .files
        .iter()
        .map(|f| ModelFileView {
            name: f.name.clone(),
            url: f.url.clone(),
            size_bytes: f.size,
            pinned_sha256: f.sha256.clone(),
            sha256: hashes.as_ref().and_then(|h| h.get(&f.name).cloned()),
        })
        .collect();
    let progress = live.progress.clone().or_else(|| {
        matches!(state, ModelItemState::Paused | ModelItemState::Failed)
            .then(|| store.partial(spec))
            .flatten()
            .map(|(file, done)| ModelProgressView {
                total: spec.files.iter().find(|f| f.name == file).map(|f| f.size),
                file,
                done,
            })
    });
    ModelItem {
        id: spec.id.clone(),
        kind: spec.kind,
        name: spec.name.clone(),
        license: spec.license.clone(),
        source: spec.source.clone(),
        size_bytes: spec.size(),
        target: spec.target(store.paths()).display().to_string(),
        files,
        state,
        pinned: spec.pinned(),
        confirmed: spec.confirmed,
        downloadable: spec.downloadable(),
        note: spec.note.clone(),
        progress,
        error: live.error.clone(),
        active,
        card: crate::cards::card(&spec.id),
        license_card: crate::licenses::card(&spec.id),
    }
}
