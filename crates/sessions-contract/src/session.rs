//! Metadane sesji, zapytania katalogu i podsumowania dla panelu Sesje.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{AgentId, ProjectId, SessionId};

/// Tag prywatności sesji (PLAN §5.5): egzekwuje go Router; agentki go nie zmieniają.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyTag {
    /// Zwykła sesja.
    #[default]
    Normal,
    /// Prywatna: bez tras „może trenować” i jurysdykcji CN.
    Private,
    /// Wyłącznie modele lokalne.
    LocalOnly,
}

/// Szablon sesji (PLAN §11).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionTemplate {
    /// Pusta sesja.
    #[default]
    Empty,
    /// Kodowanie.
    Coding,
    /// Research.
    Research,
    /// Asystent głosowy.
    VoiceAssistant,
    /// Administracja PC.
    PcAdmin,
}

/// Metadane sesji (katalog `index.db`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionMeta {
    /// Identyfikator (UUIDv7 — sortowalny w czasie; w atrapie deterministyczny).
    pub id: SessionId,
    /// Tytuł.
    pub title: String,
    /// Szablon, z którego powstała sesja.
    pub template: SessionTemplate,
    /// Polityka modeli (referencja profilu routera, np. `auto`).
    pub model_policy: String,
    /// Aktywne agentki.
    pub agents: Vec<AgentId>,
    /// Tag prywatności.
    pub privacy: PrivacyTag,
    /// Skażona treścią niezaufaną; flaga tylko rośnie (reset = nowa sesja).
    pub tainted: bool,
    /// Katalog roboczy (`<workdir_root>\<nazwa>`).
    pub workdir: PathBuf,
    /// Przypięta na górze listy.
    pub pinned: bool,
    /// Zarchiwizowana (ukryta z domyślnej listy).
    pub archived: bool,
    /// W koszu logicznym (przed ostatecznym usunięciem).
    pub trashed: bool,
    /// Projekt (folder).
    pub project: Option<ProjectId>,
    /// Tagi (posortowane, bez duplikatów).
    pub tags: Vec<String>,
    /// Utworzenie.
    pub created_at: DateTime<Utc>,
    /// Ostatnia zmiana metadanych.
    pub updated_at: DateTime<Utc>,
}

/// Parametry nowej sesji.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NewSession {
    /// Tytuł (pusty → [`crate::DEFAULT_TITLE`]).
    pub title: String,
    /// Szablon.
    pub template: SessionTemplate,
    /// Polityka modeli (pusta → `auto`).
    pub model_policy: String,
    /// Agentki.
    pub agents: Vec<AgentId>,
    /// Tag prywatności.
    pub privacy: PrivacyTag,
    /// Katalog roboczy; `None` → `<workdir_root>\<nazwa z tytułu>` (unikalna).
    pub workdir: Option<PathBuf>,
    /// Projekt.
    pub project: Option<ProjectId>,
    /// Tagi.
    pub tags: Vec<String>,
}

/// Zmiana metadanych (pola `None` bez zmian). Flagi `tainted` i `trashed` mają osobne operacje.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionPatch {
    /// Nowy tytuł (katalog roboczy się nie zmienia).
    pub title: Option<String>,
    /// Polityka modeli.
    pub model_policy: Option<String>,
    /// Agentki.
    pub agents: Option<Vec<AgentId>>,
    /// Tag prywatności.
    pub privacy: Option<PrivacyTag>,
    /// Katalog roboczy.
    pub workdir: Option<PathBuf>,
    /// Przypięcie.
    pub pinned: Option<bool>,
    /// Archiwizacja.
    pub archived: Option<bool>,
    /// Projekt (`Some(None)` usuwa przypisanie).
    pub project: Option<Option<ProjectId>>,
    /// Tagi (zastępują dotychczasowe).
    pub tags: Option<Vec<String>>,
}

/// Porządek listy sesji; przypięte zawsze na górze, remisy rozstrzyga `id`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionSort {
    /// Ostatnia aktywność (ostatnia tura albo utworzenie), malejąco.
    #[default]
    Recent,
    /// Data utworzenia, malejąco.
    Created,
    /// Tytuł (bez diakrytyków i wielkości liter), rosnąco.
    Title,
}

/// Zapytanie o listę sesji (panel Sesje).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionQuery {
    /// Fragment tytułu (bez diakrytyków i wielkości liter: „zolc” pasuje do „Żółć”).
    pub text: Option<String>,
    /// Czy pokazać zarchiwizowane.
    pub include_archived: bool,
    /// `true` → wyłącznie sesje w koszu; `false` → bez kosza.
    pub trashed: bool,
    /// Tylko sesje projektu.
    pub project: Option<ProjectId>,
    /// Tylko sesje z tagiem.
    pub tag: Option<String>,
    /// Porządek.
    pub sort: SessionSort,
}

/// Pozycja listy sesji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionSummary {
    /// Metadane.
    pub meta: SessionMeta,
    /// Liczba tur.
    pub turns: u64,
    /// Nieprzeczytane tury (inne niż użytkownika od ostatniego `mark_read`).
    pub unread: u64,
    /// Kropka aktywności (sesja pracuje w tle; stan ulotny, nie trafia do bazy).
    pub active: bool,
    /// Czas ostatniej tury.
    pub last_turn_at: Option<DateTime<Utc>>,
}

impl SessionSummary {
    /// Znacznik czasu dla sortowania „ostatnia aktywność”.
    pub fn activity_at(&self) -> DateTime<Utc> {
        self.last_turn_at.unwrap_or(self.meta.created_at)
    }
}

impl SessionMeta {
    /// Buduje metadane nowej sesji (wspólne dla `-impl` i `-fake`): tytuł przycięty (pusty →
    /// [`crate::DEFAULT_TITLE`]), polityka pusta → `auto`, tagi znormalizowane, katalog roboczy
    /// `<workdir_root>\<nazwa>` unikalny względem `taken_dirs` (nazw katalogów innych sesji).
    pub fn from_new(
        new: NewSession,
        id: SessionId,
        now: DateTime<Utc>,
        workdir_root: &std::path::Path,
        taken_dirs: &[String],
    ) -> Self {
        let title = match new.title.trim() {
            "" => crate::naming::DEFAULT_TITLE.to_owned(),
            t => t.to_owned(),
        };
        let workdir = new.workdir.unwrap_or_else(|| {
            let base = crate::naming::session_dir_name(&title, &id);
            workdir_root.join(crate::naming::unique_dir_name(&base, taken_dirs))
        });
        let model_policy = match new.model_policy.trim() {
            "" => "auto".to_owned(),
            p => p.to_owned(),
        };
        Self {
            id,
            title,
            template: new.template,
            model_policy,
            agents: new.agents,
            privacy: new.privacy,
            tainted: false,
            workdir,
            pinned: false,
            archived: false,
            trashed: false,
            project: new.project,
            tags: normalize_tags(&new.tags),
            created_at: now,
            updated_at: now,
        }
    }

    /// Stosuje zmianę metadanych (pusty tytuł jest ignorowany).
    pub fn apply(&mut self, patch: SessionPatch, now: DateTime<Utc>) {
        if let Some(title) = patch
            .title
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
        {
            self.title = title;
        }
        if let Some(policy) = patch.model_policy {
            self.model_policy = policy;
        }
        if let Some(agents) = patch.agents {
            self.agents = agents;
        }
        if let Some(privacy) = patch.privacy {
            self.privacy = privacy;
        }
        if let Some(workdir) = patch.workdir {
            self.workdir = workdir;
        }
        if let Some(pinned) = patch.pinned {
            self.pinned = pinned;
        }
        if let Some(archived) = patch.archived {
            self.archived = archived;
        }
        if let Some(project) = patch.project {
            self.project = project;
        }
        if let Some(tags) = patch.tags {
            self.tags = normalize_tags(&tags);
        }
        self.updated_at = now.max(self.updated_at);
    }

    /// Nazwa katalogu roboczego (ostatni składnik ścieżki) — do sprawdzania unikalności.
    pub fn workdir_name(&self) -> String {
        self.workdir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// Wspólne filtrowanie i porządek listy sesji (dzielone przez `-impl` i `-fake`).
///
/// Filtry: kosz (`trashed`), archiwum, projekt, tag, fragment tytułu ([`crate::title_key`]).
/// Porządek: przypięte najpierw, potem wg [`SessionSort`], remisy rosnąco po `id`.
pub fn apply_query(list: Vec<SessionSummary>, query: &SessionQuery) -> Vec<SessionSummary> {
    let needle = query
        .text
        .as_deref()
        .map(crate::naming::title_key)
        .filter(|t| !t.trim().is_empty());
    let mut out: Vec<SessionSummary> = list
        .into_iter()
        .filter(|s| s.meta.trashed == query.trashed)
        .filter(|s| query.include_archived || query.trashed || !s.meta.archived)
        .filter(|s| query.project.is_none() || s.meta.project == query.project)
        .filter(|s| query.tag.as_ref().is_none_or(|t| s.meta.tags.contains(t)))
        .filter(|s| {
            needle
                .as_deref()
                .is_none_or(|n| crate::naming::title_key(&s.meta.title).contains(n))
        })
        .collect();
    out.sort_by(|a, b| {
        let by_sort = match query.sort {
            SessionSort::Recent => b.activity_at().cmp(&a.activity_at()),
            SessionSort::Created => b.meta.created_at.cmp(&a.meta.created_at),
            SessionSort::Title => crate::naming::title_key(&a.meta.title)
                .cmp(&crate::naming::title_key(&b.meta.title)),
        };
        b.meta
            .pinned
            .cmp(&a.meta.pinned)
            .then(by_sort)
            .then_with(|| a.meta.id.cmp(&b.meta.id))
    });
    out
}

/// Normalizuje tagi: przycięte, bez pustych, posortowane, bez duplikatów.
pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = tags
        .iter()
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_normalized() {
        let tags = vec![" b ".into(), "a".into(), "".into(), "b".into()];
        assert_eq!(normalize_tags(&tags), vec!["a".to_owned(), "b".to_owned()]);
    }

    #[test]
    fn enums_serialize_snake_case() {
        assert_eq!(
            serde_json::to_string(&PrivacyTag::LocalOnly).unwrap(),
            "\"local_only\""
        );
        assert_eq!(
            serde_json::to_string(&SessionTemplate::VoiceAssistant).unwrap(),
            "\"voice_assistant\""
        );
    }
}
