//! Katalog roboczy sesji — zakres narzędzi agentek (`tools-fs`, `tools-shell`). Wybór wyłącznie
//! przez właściciela w UI (natywny dialog albo katalog sesji); bez katalogu agentki odpowiadają bez
//! narzędzi. Katalogi danych Alfy i deny-listy poświadczeń są odrzucane.

use std::path::{Path, PathBuf};

use compliance_contract::{DenyChecker, DenyLists};
use sessions_contract::{SessionCatalog, SessionId};

use crate::core::AppCore;
use crate::dto::{EventLevel, SessionWorkdir, TimelineKind, WorkdirChoice};
use crate::error::AppError;
use crate::ids;

impl AppCore {
    fn workdir_view(&self, session: &SessionId) -> Result<SessionWorkdir, AppError> {
        let meta = self.inner.sessions.session(session)?;
        Ok(SessionWorkdir {
            path: self.inner.store.workdir(session)?,
            default_path: meta.workdir.to_string_lossy().into_owned(),
        })
    }

    /// Sprawdza katalog wybrany na zakres narzędzi.
    fn check_workdir(&self, path: &Path) -> Result<String, AppError> {
        if !path.is_absolute() || !path.is_dir() {
            return Err(AppError::invalid(format!(
                "„{}” nie jest istniejącym katalogiem.",
                path.display()
            )));
        }
        let paths = &self.inner.paths;
        let internal = [&paths.local, &paths.config]
            .iter()
            .any(|dir| path.starts_with(dir) || dir.starts_with(path));
        let text = path.to_string_lossy().into_owned();
        let (_, env) = app_modules::broker::path_env_for(&paths.user_root);
        let deny = DenyChecker::new(DenyLists::baseline(), &env);
        if internal
            || deny.is_denied_path(&text, &env)
            || tools_common_contract::paths::has_credential_segment(&text)
        {
            return Err(AppError::forbidden(format!(
                "„{text}” zawiera dane Alfy albo poświadczenia — wybierz inny katalog roboczy."
            )));
        }
        Ok(text)
    }

    /// `sessions_workdir`.
    pub async fn sessions_workdir(&self, session_id: String) -> Result<SessionWorkdir, AppError> {
        let id = ids::session(&session_id)?;
        self.ensure_session(&id)?;
        self.workdir_view(&id)
    }

    /// `sessions_choose_workdir` ⟶ natywny dialog (`dialog`), katalog sesji (`default`) albo bez
    /// katalogu (`none` — agentki bez narzędzi).
    pub async fn sessions_choose_workdir(
        &self,
        session_id: String,
        choice: WorkdirChoice,
    ) -> Result<SessionWorkdir, AppError> {
        let id = ids::session(&session_id)?;
        self.ensure_session(&id)?;
        let picked: Option<PathBuf> = match choice {
            WorkdirChoice::None => None,
            WorkdirChoice::Default => {
                let dir = self.inner.sessions.session(&id)?.workdir;
                std::fs::create_dir_all(&dir)
                    .map_err(|e| AppError::storage(format!("{}: {e}", dir.display())))?;
                Some(dir)
            }
            WorkdirChoice::Dialog => {
                let shell = self.inner.shell.clone();
                let answer = tokio::task::spawn_blocking(move || shell.pick_folder())
                    .await
                    .map_err(|e| AppError::internal(format!("okno wyboru katalogu: {e}")))??;
                match answer {
                    Some(dir) => Some(dir),
                    // Anulowano — bez zmian.
                    None => return self.workdir_view(&id),
                }
            }
        };
        let path = match &picked {
            Some(dir) => Some(self.check_workdir(dir)?),
            None => None,
        };
        self.inner.store.set_workdir(&id, path.as_deref())?;
        let title = match &path {
            Some(p) => format!("Katalog roboczy agentek: {p}"),
            None => "Katalog roboczy wyłączony — agentki odpowiadają bez narzędzi".into(),
        };
        self.timeline_note(&id, TimelineKind::Ui, EventLevel::Info, title, None);
        self.workdir_view(&id)
    }
}
