//! Komendy `sessions_*`: lista, tworzenie, metadane, usuwanie z cofnięciem (potem
//! crypto-shredding), duplikat, eksport (⟶ transfer), wyszukiwanie FTS, szkice.

use std::collections::BTreeMap;
use std::time::Duration;

use search_contract::{Caller, DocKind, Mode, Query, Search, SessionSet};
use sessions_contract::{
    NewSession, SessionCatalog, SessionHistory, SessionId, SessionPatch, SessionQuery,
    SessionTemplate as CoreTemplate,
};

use crate::core::AppCore;
use crate::dto::{
    self, AlfaEvent, ExportResult, SessionSearchHit, SessionSummary, SessionTemplate, UndoTicket,
};
use crate::error::AppError;
use crate::ids;
use crate::settings::keys;

fn core_template(t: SessionTemplate) -> CoreTemplate {
    match t {
        SessionTemplate::Empty => CoreTemplate::Empty,
        SessionTemplate::Coding => CoreTemplate::Coding,
        SessionTemplate::Research => CoreTemplate::Research,
        SessionTemplate::Voice => CoreTemplate::VoiceAssistant,
        SessionTemplate::Admin => CoreTemplate::PcAdmin,
    }
}

const BUILTIN_AGENTS: [&str; 4] = ["alfa", "beta", "gama", "delta"];

impl AppCore {
    fn all_sessions(&self) -> Result<Vec<sessions_contract::SessionSummary>, AppError> {
        Ok(self.inner.sessions.list_sessions(&SessionQuery {
            include_archived: true,
            ..SessionQuery::default()
        })?)
    }

    /// `sessions_list`.
    pub async fn sessions_list(&self) -> Result<Vec<SessionSummary>, AppError> {
        let mut out = Vec::new();
        for s in self.all_sessions()? {
            out.push(self.session_dto(&s).await);
        }
        Ok(out)
    }

    pub(crate) async fn create_session(&self, new: NewSession) -> Result<SessionSummary, AppError> {
        let meta = self.inner.sessions.create_session(new)?;
        let summary = self.session_summary(&meta.id).await?;
        self.emit(AlfaEvent::SessionUpdated {
            session: summary.clone(),
        });
        Ok(summary)
    }

    /// `sessions_create`.
    pub async fn sessions_create(
        &self,
        template: SessionTemplate,
    ) -> Result<SessionSummary, AppError> {
        self.create_session(NewSession {
            title: template.title().to_owned(),
            template: core_template(template),
            agents: BUILTIN_AGENTS.iter().map(|a| (*a).into()).collect(),
            ..NewSession::default()
        })
        .await
    }

    async fn patch_session(&self, id: &str, patch: SessionPatch) -> Result<(), AppError> {
        let id = ids::session(id)?;
        self.inner.sessions.update_session(&id, patch)?;
        self.announce_session(&id).await;
        Ok(())
    }

    /// `sessions_rename`.
    pub async fn sessions_rename(&self, session_id: String, title: String) -> Result<(), AppError> {
        let title = title.trim().to_owned();
        if title.is_empty() || title.chars().count() > 200 {
            return Err(AppError::invalid(
                "Tytuł sesji musi mieć od 1 do 200 znaków.",
            ));
        }
        let patch = SessionPatch {
            title: Some(title),
            ..SessionPatch::default()
        };
        self.patch_session(&session_id, patch).await
    }

    /// `sessions_set_project`: projekt sesji (pamięć projektu dzielą sesje tego projektu);
    /// `null`/pusty = poza projektem.
    pub async fn sessions_set_project(
        &self,
        session_id: String,
        project: Option<String>,
    ) -> Result<(), AppError> {
        let project = project
            .map(|p| p.trim().to_owned())
            .filter(|p| !p.is_empty());
        if project.as_ref().is_some_and(|p| p.chars().count() > 80) {
            return Err(AppError::invalid(
                "Nazwa projektu może mieć najwyżej 80 znaków.",
            ));
        }
        let patch = SessionPatch {
            project: Some(project.map(sessions_contract::ProjectId::new)),
            ..SessionPatch::default()
        };
        self.patch_session(&session_id, patch).await
    }

    /// `sessions_set_pinned`.
    pub async fn sessions_set_pinned(
        &self,
        session_id: String,
        pinned: bool,
    ) -> Result<(), AppError> {
        let patch = SessionPatch {
            pinned: Some(pinned),
            ..SessionPatch::default()
        };
        self.patch_session(&session_id, patch).await
    }

    /// `sessions_set_archived`.
    pub async fn sessions_set_archived(
        &self,
        session_id: String,
        archived: bool,
    ) -> Result<(), AppError> {
        let patch = SessionPatch {
            archived: Some(archived),
            ..SessionPatch::default()
        };
        self.patch_session(&session_id, patch).await
    }

    async fn undo_window(&self) -> Duration {
        if let Some(window) = self.inner.undo_window {
            return window;
        }
        let secs = self
            .config_value(keys::DELETE_UNDO)
            .await
            .and_then(|v| v.as_u64())
            .unwrap_or(10);
        Duration::from_secs(secs.clamp(1, 600))
    }

    /// `sessions_remove`: kosz logiczny na czas okna cofnięcia, potem usunięcie z kluczem.
    pub async fn sessions_remove(&self, session_id: String) -> Result<UndoTicket, AppError> {
        let id = ids::session(&session_id)?;
        let _guard = self.lock_session(&id).await;
        self.finalize_generation(&id).await;
        self.inner.sessions.trash_session(&id)?;
        self.emit(AlfaEvent::SessionRemoved {
            session_id: id.to_string(),
        });
        let window = self.undo_window().await;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let core = self.clone();
        let (tk, sid) = (token.clone(), id.clone());
        let task = tokio::spawn(async move {
            tokio::time::sleep(window).await;
            let pending = core.rt().trash.remove(&tk);
            if pending.is_some() {
                core.shred_session(&sid);
            }
        });
        self.rt()
            .trash
            .insert(token.clone(), (id, task.abort_handle()));
        let expires = chrono::Utc::now()
            + chrono::Duration::from_std(window).unwrap_or_else(|_| chrono::Duration::seconds(10));
        Ok(UndoTicket {
            token,
            expires_at: dto::iso(expires),
        })
    }

    /// Ostateczne usunięcie: klucz z sejfu (crypto-shredding), pliki bazy, wpis katalogu.
    pub(crate) fn shred_session(&self, id: &SessionId) {
        // Najpierw kopie w zakresach szerszych i pochodne (kaskada), potem baza sesji.
        if let Err(e) = self.inner.memory.forget_session(id) {
            tracing::error!(sesja = %id, error = %e.message, "zapomnienie pamięci sesji nie powiodło się");
        }
        self.inner.sessions.close_session(id);
        match self.inner.sessions.delete_session(id) {
            Ok(report) => tracing::info!(sesja = %id, klucz = report.key_deleted, "sesja usunięta"),
            Err(e) => tracing::error!(sesja = %id, error = %e, "usuwanie sesji nie powiodło się"),
        }
        self.inner.store.forget(id);
    }

    /// `sessions_undo_remove`.
    pub async fn sessions_undo_remove(&self, token: String) -> Result<(), AppError> {
        let Some((id, task)) = self.rt().trash.remove(&token) else {
            return Err(AppError::not_found("Okno cofnięcia usunięcia minęło."));
        };
        task.abort();
        self.inner.sessions.restore_session(&id)?;
        self.announce_session(&id).await;
        Ok(())
    }

    /// `sessions_duplicate_as_template`: nowa pusta sesja z tymi samymi ustawieniami.
    pub async fn sessions_duplicate_as_template(
        &self,
        session_id: String,
    ) -> Result<SessionSummary, AppError> {
        let id = ids::session(&session_id)?;
        let meta = self.inner.sessions.session(&id)?;
        self.create_session(NewSession {
            title: format!("{} (szablon)", meta.title),
            template: meta.template,
            model_policy: meta.model_policy,
            agents: meta.agents,
            privacy: meta.privacy,
            workdir: None,
            project: meta.project,
            tags: meta.tags,
        })
        .await
    }

    /// `sessions_export` ⟶ moduł `transfer`.
    pub async fn sessions_export(&self, session_id: String) -> Result<ExportResult, AppError> {
        let id = ids::session(&session_id)?;
        self.ensure_session(&id)?;
        self.inner.transfer.export_session(&id).await
    }

    /// `sessions_search`: FTS5 w treści tur wszystkich sesji + dopasowanie tytułu.
    pub async fn sessions_search(&self, query: String) -> Result<Vec<SessionSearchHit>, AppError> {
        let text = query.trim();
        if text.is_empty() {
            return Ok(Vec::new());
        }
        let sessions: BTreeMap<String, String> = self
            .all_sessions()?
            .into_iter()
            .map(|s| (s.meta.id.to_string(), s.meta.title))
            .collect();
        let hits = self.inner.search.query(
            &Query {
                text: text.to_owned(),
                sessions: SessionSet::All,
                mode: Mode::Fts,
                limit: 50,
                kinds: vec![DocKind::Turn],
            },
            &Caller::Owner,
        )?;
        let mut out: Vec<SessionSearchHit> = Vec::new();
        for hit in hits {
            let Some(title) = sessions.get(hit.session.as_str()) else {
                continue;
            };
            let turn = hit.doc.key.parse::<u64>().ok().filter(|n| *n > 0);
            out.push(SessionSearchHit {
                session_id: hit.session.to_string(),
                title: title.clone(),
                snippet: hit.snippet.text,
                turn_id: turn.map(|n| ids::turn_dto(&hit.session, sessions_contract::TurnId(n))),
            });
        }
        let titled = self.inner.sessions.list_sessions(&SessionQuery {
            text: Some(text.to_owned()),
            include_archived: true,
            ..SessionQuery::default()
        })?;
        for s in titled {
            if !out.iter().any(|h| h.session_id == s.meta.id.as_str()) {
                out.push(SessionSearchHit {
                    session_id: s.meta.id.to_string(),
                    title: s.meta.title,
                    snippet: String::new(),
                    turn_id: None,
                });
            }
        }
        Ok(out)
    }

    /// `sessions_mark_read`.
    pub async fn sessions_mark_read(&self, session_id: String) -> Result<(), AppError> {
        let id = ids::session(&session_id)?;
        self.inner.sessions.mark_read(&id)?;
        self.announce_session(&id).await;
        Ok(())
    }

    /// `sessions_get_draft`.
    pub async fn sessions_get_draft(&self, session_id: String) -> Result<String, AppError> {
        let id = ids::session(&session_id)?;
        Ok(self.inner.sessions.draft(&id)?.unwrap_or_default())
    }

    /// `sessions_save_draft`.
    pub async fn sessions_save_draft(
        &self,
        session_id: String,
        text: String,
    ) -> Result<(), AppError> {
        let id = ids::session(&session_id)?;
        Ok(self.inner.sessions.save_draft(&id, &text)?)
    }

    /// Tag prywatności sesji (`normal` / `private` / `local_only`) — trasy Routera, eksport.
    /// Na razie API rdzenia (powłoka, testy); przełącznik w UI — kolejna fala `ui-shell`.
    pub async fn sessions_set_privacy(
        &self,
        session_id: String,
        privacy: sessions_contract::PrivacyTag,
    ) -> Result<(), AppError> {
        let id = ids::session(&session_id)?;
        self.ensure_session(&id)?;
        let patch = SessionPatch {
            privacy: Some(privacy),
            ..SessionPatch::default()
        };
        self.inner.sessions.update_session(&id, patch)?;
        self.announce_session(&id).await;
        Ok(())
    }
}
