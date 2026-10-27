//! Katalog roboczy sesji — zakres narzędzi agentek (`tools-fs`, `tools-shell`). Wybór wyłącznie
//! przez właściciela w UI (natywny dialog albo katalog sesji); bez katalogu agentki odpowiadają bez
//! narzędzi. Katalogi danych Alfy i deny-listy poświadczeń są odrzucane — także przez dowiązania
//! (`app_modules::workdir`).

use app_files::projects as p;
use sessions_contract::{ProjectId, SessionCatalog, SessionId};

use crate::core::AppCore;
use crate::dto::{EventLevel, ProjectInfo, SessionWorkdir, TimelineKind, WorkdirChoice};
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
        let path: Option<String> = match choice {
            WorkdirChoice::None => None,
            WorkdirChoice::Default => {
                let dir = self.inner.sessions.session(&id)?.workdir;
                Some(p::default_workdir(&dir, &self.inner.paths)?)
            }
            WorkdirChoice::Dialog => {
                match p::pick_dir(self.inner.shell.clone(), &self.inner.paths).await? {
                    Some(dir) => Some(dir),
                    None => return self.workdir_view(&id),
                }
            }
        };
        self.inner.store.set_workdir(&id, path.as_deref())?;
        let title = match &path {
            Some(p) => format!("Katalog roboczy agentek: {p}"),
            None => "Katalog roboczy wyłączony — agentki odpowiadają bez narzędzi".into(),
        };
        self.timeline_note(&id, TimelineKind::Ui, EventLevel::Info, title, None);
        self.workdir_view(&id)
    }

    async fn replace_project(&self, from: &str, to: Option<ProjectId>) -> Result<(), AppError> {
        let all = self.all_sessions()?;
        for id in p::patch_sessions_project(&*self.inner.sessions, &all, from, to)? {
            self.announce_session(&id).await;
        }
        Ok(())
    }

    /// `projects_list`.
    pub async fn projects_list(&self) -> Result<Vec<ProjectInfo>, AppError> {
        let all = self.all_sessions()?;
        Ok(p::list_for(&self.inner.paths.state(), &all))
    }

    /// `projects_pick_dir`.
    pub async fn projects_pick_dir(&self) -> Result<Option<String>, AppError> {
        p::pick_dir(self.inner.shell.clone(), &self.inner.paths).await
    }

    /// `projects_save`.
    pub async fn projects_save(
        &self,
        name: String,
        workdir: Option<String>,
    ) -> Result<Vec<ProjectInfo>, AppError> {
        let state = self.inner.paths.state();
        let (clean, dir) = p::upsert_project(&state, &self.inner.paths, &name, workdir)?;
        for s in self.all_sessions()? {
            if let (Some(d), Some(pr)) = (dir.as_deref(), &s.meta.project)
                && pr.0 == clean
            {
                self.inner.store.set_workdir(&s.meta.id, Some(d))?;
            }
        }
        self.projects_list().await
    }

    /// `projects_rename`.
    pub async fn projects_rename(
        &self,
        old_name: String,
        new_name: String,
    ) -> Result<Vec<ProjectInfo>, AppError> {
        let (old_c, new_c) = p::rename_entry(&self.inner.paths.state(), &old_name, &new_name)?;
        if old_c != new_c {
            self.replace_project(&old_c, Some(ProjectId::new(new_c)))
                .await?;
        }
        self.projects_list().await
    }

    /// `projects_remove`.
    pub async fn projects_remove(&self, name: String) -> Result<Vec<ProjectInfo>, AppError> {
        let clean = p::remove_entry(&self.inner.paths.state(), &name)?;
        self.replace_project(&clean, None).await?;
        self.projects_list().await
    }
}
