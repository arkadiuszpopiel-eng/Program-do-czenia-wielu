//! Katalog roboczy sesji — zakres narzędzi agentek (`tools-fs`, `tools-shell`). Wybór wyłącznie
//! przez właściciela w UI (natywny dialog albo katalog sesji); bez katalogu agentki odpowiadają bez
//! narzędzi. Katalogi danych Alfy i deny-listy poświadczeń są odrzucane — także przez dowiązania
//! (`app_modules::workdir`).

use std::path::PathBuf;

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
            Some(dir) => Some(app_modules::workdir::check_workdir(dir, &self.inner.paths)?),
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
