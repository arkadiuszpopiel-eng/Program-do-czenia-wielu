//! Komendy `files_*`: artefakty sesji, podgląd (tekst jako tekst, obraz przez protokół zasobów),
//! akcje (⟶ powłoka jako użytkownik).

use artifacts_contract::{ArtifactAction as CoreAction, Artifacts, Origin, Preview};

use crate::core::AppCore;
use crate::dto::{self, ArtifactAction, ArtifactInfo, ArtifactPreview};
use crate::error::AppError;
use crate::ids;

/// Limit podglądu tekstu (bajty).
const PREVIEW_BYTES: usize = 64 * 1024;

/// URL protokołu zasobów Tauri dla ścieżki (`asset://localhost/<ścieżka zakodowana>`).
pub fn asset_url(path: &std::path::Path) -> String {
    let raw = path.to_string_lossy();
    let mut out = String::from("asset://localhost/");
    for b in raw.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(*b));
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

impl AppCore {
    /// `files_list`.
    pub async fn files_list(&self, session_id: String) -> Result<Vec<ArtifactInfo>, AppError> {
        let id = ids::session(&session_id)?;
        self.ensure_session(&id)?;
        let list = self.inner.artifacts.list(&id)?;
        Ok(list
            .iter()
            .filter_map(|a| {
                let first = a.versions.first()?;
                let latest = a.latest()?;
                Some(ArtifactInfo {
                    id: ids::artifact_dto(&id, &a.id),
                    session_id: id.to_string(),
                    name: a.name.clone(),
                    path: latest.path.to_string_lossy().into_owned(),
                    size_bytes: latest.bytes,
                    mime: latest.mime.clone(),
                    created_at: dto::iso(first.created_at),
                    agent: match &a.origin {
                        Origin::Agent { agent } => Some(agent.to_string()),
                        _ => None,
                    },
                    versions: u32::try_from(a.versions.len()).unwrap_or(u32::MAX),
                })
            })
            .collect())
    }

    /// `files_preview`.
    pub async fn files_preview(&self, artifact_id: String) -> Result<ArtifactPreview, AppError> {
        let (session, artifact) = ids::parse_artifact(&artifact_id)?;
        let found = self.inner.artifacts.get(&session, &artifact)?;
        let Some(latest) = found.latest() else {
            return Ok(ArtifactPreview::None);
        };
        if latest.mime.starts_with("image/") {
            return Ok(ArtifactPreview::Image {
                src: asset_url(&latest.path),
            });
        }
        match self
            .inner
            .artifacts
            .preview(&session, &artifact, None, PREVIEW_BYTES)?
        {
            Preview::Text { text, truncated } => Ok(ArtifactPreview::Text { text, truncated }),
            Preview::Binary { .. } => Ok(ArtifactPreview::None),
        }
    }

    /// `files_act` ⟶ Otwórz · Pokaż w Eksploratorze · Kopiuj · Zapisz jako (wykonuje powłoka).
    pub async fn files_act(
        &self,
        artifact_id: String,
        action: ArtifactAction,
    ) -> Result<(), AppError> {
        let (session, artifact) = ids::parse_artifact(&artifact_id)?;
        let core_action = match action {
            ArtifactAction::Open => CoreAction::Open,
            ArtifactAction::Reveal => CoreAction::Reveal,
            ArtifactAction::Copy => CoreAction::CopyAsFile,
            ArtifactAction::SaveAs => {
                let found = self.inner.artifacts.get(&session, &artifact)?;
                let text =
                    match self
                        .inner
                        .artifacts
                        .preview(&session, &artifact, None, usize::MAX)?
                    {
                        Preview::Text { text, .. } => text,
                        Preview::Binary { .. } => {
                            return Err(AppError::unavailable(
                                "Zapis pliku binarnego jako…",
                                "shell-integration",
                            ));
                        }
                    };
                self.inner.shell.save_text_as(&found.name, &text)?;
                return Ok(());
            }
        };
        let intent = self
            .inner
            .artifacts
            .intent(&session, &artifact, None, core_action)?;
        // Wykonawca sprawdza, że plik nie zmienił się od rejestracji (SPEC artifacts).
        let facts = artifacts_contract::read_file_facts(&intent.path, 0)?;
        if facts.sha256 != intent.sha256 {
            return Err(AppError::invalid(
                "Plik zmienił się od zarejestrowania — odśwież listę plików.",
            ));
        }
        self.inner.shell.artifact_action(&intent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_urls_are_percent_encoded() {
        let url = asset_url(std::path::Path::new("C:\\Users\\Ty\\obraz 1.png"));
        assert_eq!(url, "asset://localhost/C%3A%5CUsers%5CTy%5Cobraz%201.png");
    }
}
