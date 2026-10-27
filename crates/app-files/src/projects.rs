//! Rejestr projektów (`projects.json` w `paths.state()`) — nazwa, katalog roboczy i licznik rozmów.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use app_api::AppError;
use app_api::dto::ProjectInfo;
use app_api::paths::AppPaths;
use app_api::ports::ShellPort;
use app_modules::workdir::check_workdir;
use serde::{Deserialize, Serialize};
use sessions_contract::{ProjectId, SessionId, SessionPatch, SessionSummary, Sessions};

/// Maksymalna długość nazwy projektu (w znakach Unicode).
pub const MAX_PROJECT_NAME_CHARS: usize = 80;

/// Maksymalna liczba zapisanych projektów.
pub const MAX_PROJECTS: usize = 200;

const PROJECTS_FILE: &str = "projects.json";

/// Zapisany projekt w `projects.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectEntry {
    /// Nazwa projektu.
    pub name: String,
    /// Opcjonalny katalog roboczy projektu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
}

fn storage_err(e: impl std::fmt::Display) -> AppError {
    AppError::storage(format!("projects.json: {e}"))
}

fn projects_path(root: &Path) -> PathBuf {
    root.join(PROJECTS_FILE)
}

/// Wczytuje zapisane projekty z `projects.json` (brak pliku = pusta lista).
pub fn load_entries(root: &Path) -> Vec<ProjectEntry> {
    let path = projects_path(root);
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

/// Zapisuje listę projektów atomowo przez plik tymczasowy `.json.tmp`.
pub fn save_entries(root: &Path, items: &[ProjectEntry]) -> Result<(), AppError> {
    std::fs::create_dir_all(root).map_err(storage_err)?;
    let path = projects_path(root);
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(items).map_err(storage_err)?;
    std::fs::write(&tmp, json).map_err(storage_err)?;
    std::fs::rename(&tmp, &path).map_err(storage_err)
}

/// Sprawdza i przycina nazwę projektu (1..=80 znaków).
pub fn validate_name(raw: &str) -> Result<String, AppError> {
    let clean = raw.trim();
    let count = clean.chars().count();
    if count == 0 || count > MAX_PROJECT_NAME_CHARS {
        return Err(AppError::invalid(
            "Nazwa projektu musi mieć od 1 do 80 znaków.",
        ));
    }
    Ok(clean.to_string())
}

/// Wyciąga domyślną nazwę projektu z ostatniego członu ścieżki katalogu.
pub fn folder_default_name(path: &str) -> Option<String> {
    let p = Path::new(path.trim());
    let raw = p
        .file_name()
        .and_then(|s| s.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    let truncated: String = raw.chars().take(MAX_PROJECT_NAME_CHARS).collect();
    let trimmed = truncated.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Zwraca zapisany katalog roboczy projektu (jeśli projekt ma przypisany katalog).
pub fn project_workdir(root: &Path, name: &str) -> Option<String> {
    let clean = name.trim();
    load_entries(root)
        .into_iter()
        .find(|p| p.name == clean)
        .and_then(|p| p.workdir)
}

/// Dodaje nowy projekt lub aktualizuje katalog istniejącego projektu.
/// Gdy `name` jest puste, a podano `workdir`, wyprowadza nazwę z nazwy katalogu.
pub fn upsert_project(
    root: &Path,
    app_paths: &AppPaths,
    name: &str,
    workdir: Option<String>,
) -> Result<(String, Option<String>), AppError> {
    let clean_dir = match workdir.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(dir) => Some(check_workdir(Path::new(dir), app_paths)?),
        None => None,
    };
    let candidate = if name.trim().is_empty() {
        clean_dir
            .as_deref()
            .and_then(folder_default_name)
            .unwrap_or_default()
    } else {
        name.trim().to_string()
    };
    let clean_name = validate_name(&candidate)?;
    let mut entries = load_entries(root);
    if let Some(existing) = entries.iter_mut().find(|p| p.name == clean_name) {
        existing.workdir = clean_dir.clone();
    } else {
        if entries.len() >= MAX_PROJECTS {
            return Err(AppError::invalid("Osiągnięto limit 200 projektów."));
        }
        entries.push(ProjectEntry {
            name: clean_name.clone(),
            workdir: clean_dir.clone(),
        });
    }
    save_entries(root, &entries)?;
    Ok((clean_name, clean_dir))
}

/// Zmienia nazwę projektu w `projects.json`.
pub fn rename_entry(
    root: &Path,
    old_name: &str,
    new_name: &str,
) -> Result<(String, String), AppError> {
    let old_clean = validate_name(old_name)?;
    let new_clean = validate_name(new_name)?;
    if old_clean == new_clean {
        return Ok((old_clean, new_clean));
    }
    let mut entries = load_entries(root);
    let old_workdir = entries
        .iter()
        .find(|p| p.name == old_clean)
        .and_then(|p| p.workdir.clone());
    entries.retain(|p| p.name != old_clean);
    if let Some(target) = entries.iter_mut().find(|p| p.name == new_clean) {
        if target.workdir.is_none() {
            target.workdir = old_workdir;
        }
    } else {
        entries.push(ProjectEntry {
            name: new_clean.clone(),
            workdir: old_workdir,
        });
    }
    save_entries(root, &entries)?;
    Ok((old_clean, new_clean))
}

/// Usuwa projekt z `projects.json`.
pub fn remove_entry(root: &Path, name: &str) -> Result<String, AppError> {
    let clean = validate_name(name)?;
    let mut entries = load_entries(root);
    entries.retain(|p| p.name != clean);
    save_entries(root, &entries)?;
    Ok(clean)
}

/// Łączy projekty zapisane w `projects.json` z projektami przypisanymi do rozmów.
pub fn merge_projects<'a>(
    saved: Vec<ProjectEntry>,
    session_projects: impl IntoIterator<Item = Option<&'a str>>,
) -> Vec<ProjectInfo> {
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for proj in session_projects.into_iter().flatten() {
        let trimmed = proj.trim();
        if !trimmed.is_empty() {
            *counts.entry(trimmed.to_string()).or_insert(0) += 1;
        }
    }
    let mut by_name: BTreeMap<String, Option<String>> = BTreeMap::new();
    for entry in saved {
        let trimmed = entry.name.trim();
        if !trimmed.is_empty() {
            by_name.insert(trimmed.to_string(), entry.workdir);
        }
    }
    for name in counts.keys() {
        by_name.entry(name.clone()).or_insert(None);
    }
    let mut out: Vec<ProjectInfo> = by_name
        .into_iter()
        .map(|(name, workdir)| {
            let session_count = counts.get(&name).copied().unwrap_or(0);
            ProjectInfo {
                name,
                workdir,
                session_count,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });
    out
}

/// Zwraca listę projektów połączoną z listą sesji.
pub fn list_for(root: &Path, sessions: &[SessionSummary]) -> Vec<ProjectInfo> {
    let names = sessions
        .iter()
        .map(|s| s.meta.project.as_ref().map(|p| p.0.as_str()));
    merge_projects(load_entries(root), names)
}

/// Przepisuje nazwę projektu we wszystkich pasujących sesjach i zwraca ich identyfikatory.
pub fn patch_sessions_project(
    sessions: &dyn Sessions,
    all: &[SessionSummary],
    from: &str,
    to: Option<ProjectId>,
) -> Result<Vec<SessionId>, AppError> {
    let mut touched = Vec::new();
    for s in all {
        if s.meta.project.as_ref().map(|p| p.0.as_str()) == Some(from) {
            let patch = SessionPatch {
                project: Some(to.clone()),
                ..SessionPatch::default()
            };
            sessions
                .update_session(&s.meta.id, patch)
                .map_err(AppError::from)?;
            touched.push(s.meta.id.clone());
        }
    }
    Ok(touched)
}

/// Otwiera natywny dialog wyboru katalogu projektu i waliduje ścieżkę.
pub async fn pick_dir(
    shell: Arc<dyn ShellPort>,
    paths: &AppPaths,
) -> Result<Option<String>, AppError> {
    let picked = tokio::task::spawn_blocking(move || shell.pick_folder())
        .await
        .map_err(|e| AppError::internal(format!("okno wyboru katalogu projektu: {e}")))??;
    picked.map(|dir| check_workdir(&dir, paths)).transpose()
}

/// Tworzy domyślny katalog sesji na dysku i zwraca kanoniczną ścieżkę.
pub fn default_workdir(dir: &Path, paths: &AppPaths) -> Result<String, AppError> {
    std::fs::create_dir_all(dir)
        .map_err(|e| AppError::storage(format!("{}: {e}", dir.display())))?;
    check_workdir(dir, paths)
}

/// Kiedy ostatnio sprawdzano konto po błędzie w rozmowie (konto → chwila; odstęp 60 s).
#[derive(Default)]
pub struct Retests(std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>>);

impl Retests {
    /// `true` — wolno sprawdzić teraz (i zapisuje tę chwilę).
    pub fn allow(&self, account: &str, now: std::time::Instant) -> bool {
        let mut seen = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if seen
            .get(account)
            .is_some_and(|last| now.duration_since(*last) < std::time::Duration::from_secs(60))
        {
            return false;
        }
        seen.insert(account.to_owned(), now);
        true
    }
}

/// Czas odstawienia konta po limicie zapytań (60 s domyślnie, maks. 1 h).
pub fn bench_until(retry_at: Option<&str>) -> chrono::DateTime<chrono::Utc> {
    let now = chrono::Utc::now();
    let default_bench = chrono::Duration::seconds(60);
    let max_bench = chrono::Duration::hours(1);
    retry_at
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .map_or(now + default_bench, |t| t.with_timezone(&chrono::Utc))
        .clamp(now, now + max_bench)
}

/// Komunikat o przełączeniu na konto zapasowe.
pub fn switched_toast(from: &str, to: &str, why_pl: &str, why_en: &str) -> app_api::dto::AlfaEvent {
    app_api::dto::AlfaEvent::Toast {
        kind: app_api::dto::ToastKind::Info,
        message: app_api::dto::LocalizedText::new(
            format!(
                "Konto „{from}” {why_pl} — kolejne wiadomości pójdą przez konto zapasowe „{to}”."
            ),
            format!(
                "Account “{from}” {why_en} — next messages will go through the backup account “{to}”."
            ),
        ),
    }
}

/// `router.fallback` między kontami tego samego dostawcy: konto, które zawiodło, i błąd tury,
/// jaki dałoby bez konta zapasowego (`None` — przejściowy albo nie między kontami).
pub fn handover(
    p: &serde_json::Value,
    brain: impl Fn(&str) -> Option<app_api::ports::BrainTarget>,
) -> Option<(String, app_api::dto::TurnError)> {
    let target = |key: &str| p[key].as_str().and_then(&brain);
    let (from, to) = (target("from")?, target("to")?);
    let (account, other) = (from.account?, to.account?);
    if account == other || from.provider_id != to.provider_id {
        return None;
    }
    // `FallbackCause::Error { kind }` → `{"cause":"error","kind":{"kind":"rate_limited",…}}`.
    let cause = &p["cause"]["kind"];
    let code = match cause["kind"].as_str()? {
        "rate_limited" => app_api::dto::TurnErrorCode::RateLimited,
        // Przejściowe (przeciążenie, sieć, czas, 5xx) — konto zostaje, bez komunikatu.
        "overloaded" | "network" | "timeout" => return None,
        "server" if cause["status"].as_u64().is_some_and(|s| s >= 500) => return None,
        _ => app_api::dto::TurnErrorCode::Provider,
    };
    let ms = cause["retry_after_ms"]
        .as_u64()
        .and_then(|ms| i64::try_from(ms).ok());
    let retry_at =
        ms.map(|ms| (chrono::Utc::now() + chrono::Duration::milliseconds(ms)).to_rfc3339());
    let message = String::new();
    Some((
        account,
        app_api::dto::TurnError {
            code,
            message,
            retry_at,
            provider: None,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_rename_and_remove_projects() {
        let dir = tempfile::tempdir().expect("tempdir");
        let app = AppPaths::under(dir.path());
        let work = app.user_root.join("MojProjekt");
        std::fs::create_dir_all(&work).expect("work");
        let work_str = work.to_string_lossy().into_owned();
        let state = app.state();

        let (name, saved_dir) =
            upsert_project(&state, &app, "", Some(work_str.clone())).expect("upsert");
        assert_eq!(name, "MojProjekt");
        assert_eq!(saved_dir.as_deref(), Some(work_str.as_str()));

        let list = merge_projects(
            load_entries(&state),
            [Some("MojProjekt"), Some("Sklep"), None],
        );
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "MojProjekt");
        assert_eq!(list[0].session_count, 1);
        assert_eq!(list[0].workdir.as_deref(), Some(work_str.as_str()));
        assert_eq!(list[1].name, "Sklep");
        assert_eq!(list[1].session_count, 1);

        rename_entry(&state, "MojProjekt", "Nowy Projekt").expect("rename");
        assert_eq!(
            project_workdir(&state, "Nowy Projekt").as_deref(),
            Some(work_str.as_str())
        );

        remove_entry(&state, "Nowy Projekt").expect("remove");
        assert!(load_entries(&state).is_empty());
    }
}
