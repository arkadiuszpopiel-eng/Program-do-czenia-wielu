//! Komendy umiejętności (`skills_*`): biblioteka (stan, wersja, źródło, kwarantanna), przegląd
//! propozycji z diffem względem wersji zainstalowanej i hashem, zatwierdzenie/zwolnienie wyłącznie
//! z UI (`ApprovalOrigin::Ui`, hash = przejrzana treść), odrzucenie, wyłączenie, uruchomienie jako
//! zadanie agentki (koperta ≤ jej roli), eksport i import paczki `alfa.skills.v1` (import zawsze
//! jako propozycje — plik z zewnątrz trafia do kwarantanny).

use std::path::PathBuf;
use std::sync::Arc;

use app_api::dto::{
    AlfaEvent, ExportResult, SkillImportResult, SkillInfo, SkillOrigin, SkillReview,
    SkillStateView, TaskInfo, iso,
};
use app_api::ports::ShellPort;
use app_api::{AppError, EventHub};
use app_tasks::TasksApp;
use core_bus_contract::{BusItem, EventBus, EventFilter};
use futures_util::StreamExt;
use skills_contract::{
    ApprovalOrigin, ImportOrigin, MAX_BUNDLE_BYTES, OwnerApproval, Skill, SkillBundle, SkillError,
    SkillId, SkillRecord, SkillSource, SkillState, Skills,
};
use skills_impl::SkillsModule;

use crate::diff::diff_lines;

/// Nazwa pliku eksportu.
const EXPORT_NAME: &str = "umiejetnosci-alfa.skills.json";

fn when(ms: u64) -> String {
    let ms = i64::try_from(ms).unwrap_or(i64::MAX);
    iso(chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default())
}

fn origin(source: &SkillSource) -> (SkillOrigin, bool) {
    match source {
        SkillSource::Memory { trusted, .. } => (SkillOrigin::Memory, *trusted),
        SkillSource::User => (SkillOrigin::User, true),
        SkillSource::Import {
            origin: ImportOrigin::OwnPackage,
        } => (SkillOrigin::OwnPackage, true),
        SkillSource::Import {
            origin: ImportOrigin::External,
        } => (SkillOrigin::External, false),
    }
}

fn state(s: SkillState) -> SkillStateView {
    match s {
        SkillState::Proposed => SkillStateView::Proposed,
        SkillState::Quarantined => SkillStateView::Quarantined,
        SkillState::Installed => SkillStateView::Installed,
        SkillState::Rejected => SkillStateView::Rejected,
        SkillState::Disabled => SkillStateView::Disabled,
        SkillState::Superseded => SkillStateView::Superseded,
    }
}

/// Projekcja wersji do DTO.
pub fn info(r: &SkillRecord) -> SkillInfo {
    let (origin, trusted) = origin(&r.source);
    SkillInfo {
        id: r.skill.id.as_str().to_owned(),
        version: r.skill.version.to_string(),
        name: r.skill.name.clone(),
        description: r.skill.description.clone(),
        state: state(r.state),
        origin,
        trusted,
        hash: r.hash.clone(),
        findings: r.findings.clone(),
        keywords: r.skill.keywords.clone(),
        required_tools: r.skill.required_tools.clone(),
        required_capabilities: r.skill.required_capabilities.clone(),
        parameters: r.skill.parameters.clone(),
        proposed_at: when(r.proposed_at_ms),
        decided_at: r.decided_at_ms.map(when),
    }
}

fn skill_error(e: SkillError) -> AppError {
    match e {
        SkillError::NotFound(_) => AppError::not_found(format!("Umiejętność: {e}")),
        SkillError::Store(_) => AppError::storage(format!("Umiejętność: {e}")),
        _ => AppError::invalid(format!("Umiejętność: {e}")),
    }
}

fn version(v: &str) -> Result<semver::Version, AppError> {
    semver::Version::parse(v.trim()).map_err(|e| AppError::invalid(format!("Wersja „{v}”: {e}")))
}

fn pretty(skill: &Skill) -> String {
    serde_json::to_string_pretty(skill).unwrap_or_default()
}

/// Biblioteka umiejętności w aplikacji (moduł niezbudowany → komendy „niedostępne").
pub struct SkillsApp {
    module: Result<Arc<SkillsModule>, String>,
    tasks: Arc<TasksApp>,
    shell: Arc<dyn ShellPort>,
}

impl std::fmt::Debug for SkillsApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillsApp")
            .field("available", &self.module.is_ok())
            .finish_non_exhaustive()
    }
}

impl SkillsApp {
    /// Komendy nad uruchomionym modułem (albo błędem jego budowy).
    pub fn new(
        module: Result<Arc<SkillsModule>, String>,
        tasks: Arc<TasksApp>,
        shell: Arc<dyn ShellPort>,
    ) -> Self {
        Self {
            module,
            tasks,
            shell,
        }
    }

    /// Moduł (rejestr, `Launch::bind_skills`).
    pub fn module(&self) -> Result<Arc<SkillsModule>, String> {
        self.module.clone()
    }

    fn m(&self) -> Result<&Arc<SkillsModule>, AppError> {
        self.module
            .as_ref()
            .map_err(|_| AppError::unavailable("Umiejętności", "skills"))
    }

    fn record(&self, id: &str, version: &semver::Version) -> Result<SkillRecord, AppError> {
        self.m()?
            .list()
            .into_iter()
            .find(|r| r.skill.id.as_str() == id && &r.skill.version == version)
            .ok_or_else(|| {
                AppError::not_found(format!("Umiejętność „{id}” {version} nie istnieje."))
            })
    }

    /// `skills_list`: wszystkie wersje (najnowsze propozycje i kwarantanna na górze).
    pub fn list(&self) -> Result<Vec<SkillInfo>, AppError> {
        let mut all: Vec<SkillRecord> = self.m()?.list();
        all.sort_by_key(|r| std::cmp::Reverse(r.proposed_at_ms));
        Ok(all.iter().map(info).collect())
    }

    /// `skills_review`: diff względem wersji zainstalowanej + hash do potwierdzenia.
    pub fn review(&self, id: &str, v: &str) -> Result<SkillReview, AppError> {
        let record = self.record(id, &version(v)?)?;
        let installed = self
            .m()?
            .installed(&SkillId::new(id))
            .filter(|r| r.skill.version != record.skill.version);
        let before = installed
            .as_ref()
            .map(|r| pretty(&r.skill))
            .unwrap_or_default();
        Ok(SkillReview {
            skill: info(&record),
            previous_version: installed.map(|r| r.skill.version.to_string()),
            diff: diff_lines(&before, &pretty(&record.skill)),
        })
    }

    /// `skills_propose`: przepis właściciela (formularz/JSON) — walidacja, testy, skaner.
    pub async fn propose(&self, skill: serde_json::Value) -> Result<SkillInfo, AppError> {
        let skill: Skill = serde_json::from_value(skill)
            .map_err(|e| AppError::invalid(format!("Umiejętność: niepoprawny przepis: {e}")))?;
        self.m()?
            .propose(skill, SkillSource::User)
            .await
            .map(|r| info(&r))
            .map_err(skill_error)
    }

    fn approval(hash: &str) -> OwnerApproval {
        OwnerApproval {
            origin: ApprovalOrigin::Ui,
            reviewed_hash: hash.trim().to_owned(),
        }
    }

    /// `skills_approve`: instalacja — tylko kliknięcie w UI, hash przejrzanej wersji.
    pub async fn approve(&self, id: &str, v: &str, hash: &str) -> Result<SkillInfo, AppError> {
        self.m()?
            .approve(&SkillId::new(id), &version(v)?, Self::approval(hash))
            .await
            .map(|r| info(&r))
            .map_err(skill_error)
    }

    /// `skills_release`: zwolnienie z kwarantanny — tylko kliknięcie w UI.
    pub async fn release(&self, id: &str, v: &str, hash: &str) -> Result<SkillInfo, AppError> {
        self.m()?
            .release(&SkillId::new(id), &version(v)?, Self::approval(hash))
            .await
            .map(|r| info(&r))
            .map_err(skill_error)
    }

    /// `skills_reject`.
    pub async fn reject(&self, id: &str, v: &str) -> Result<SkillInfo, AppError> {
        self.m()?
            .reject(&SkillId::new(id), &version(v)?)
            .await
            .map(|r| info(&r))
            .map_err(skill_error)
    }

    /// `skills_disable`.
    pub async fn disable(&self, id: &str) -> Result<SkillInfo, AppError> {
        self.m()?
            .disable(&SkillId::new(id))
            .await
            .map(|r| info(&r))
            .map_err(skill_error)
    }

    /// `skills_run`: zadanie agentki w sesji z kopertą umiejętności (sprawdza wykonawczyni
    /// przy starcie: wymagane narzędzia ⊆ rola agentki, parametry wg schematu).
    pub fn run(
        &self,
        id: &str,
        session: &str,
        agent: Option<&str>,
        params: serde_json::Value,
    ) -> Result<TaskInfo, AppError> {
        let record = self.m()?.installed(&SkillId::new(id)).ok_or_else(|| {
            AppError::not_found(format!("Umiejętność „{id}” nie jest zainstalowana."))
        })?;
        skills_contract::schema::validate_params(&record.skill.parameters, &params)
            .map_err(|e| AppError::invalid(format!("Parametry umiejętności: {e}")))?;
        let call = app_agents::SkillCall {
            id: id.to_owned(),
            params,
        };
        self.tasks
            .create_skill(session, agent, &record.skill.name, &call)
    }

    /// `skills_export`: paczka zainstalowanych (natywne „Zapisz jako").
    pub async fn export(&self) -> Result<ExportResult, AppError> {
        let bundle = self.m()?.export(&[]).map_err(skill_error)?;
        let count = u64::try_from(bundle.skills.len()).unwrap_or(u64::MAX);
        let bytes = bundle.to_bytes().map_err(AppError::internal)?;
        let shell = self.shell.clone();
        let picked = tokio::task::spawn_blocking(move || shell.pick_save_path(EXPORT_NAME))
            .await
            .map_err(|e| AppError::internal(format!("okno zapisu: {e}")))??;
        let Some(path) = picked else {
            return Ok(ExportResult::Cancelled);
        };
        let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        std::fs::write(&path, bytes)
            .map_err(|e| AppError::storage(format!("{}: {e}", path.display())))?;
        Ok(ExportResult::Saved {
            path: path.to_string_lossy().into_owned(),
            files: count,
            bytes: size,
        })
    }

    /// `skills_import`: plik z zewnątrz → propozycje w kwarantannie (nigdy instalacja).
    pub async fn import(&self) -> Result<SkillImportResult, AppError> {
        let shell = self.shell.clone();
        let picked: Option<PathBuf> = tokio::task::spawn_blocking(move || shell.pick_open_path())
            .await
            .map_err(|e| AppError::internal(format!("okno otwarcia: {e}")))??;
        let Some(path) = picked else {
            return Ok(SkillImportResult {
                proposed: Vec::new(),
                skipped: Vec::new(),
            });
        };
        let size = std::fs::metadata(&path)
            .map_err(|e| AppError::storage(format!("{}: {e}", path.display())))?
            .len();
        if size > u64::try_from(MAX_BUNDLE_BYTES).unwrap_or(u64::MAX) {
            return Err(AppError::invalid("Paczka umiejętności jest za duża."));
        }
        let bytes = std::fs::read(&path)
            .map_err(|e| AppError::storage(format!("{}: {e}", path.display())))?;
        self.import_bytes(&bytes).await
    }

    /// Import treści paczki (z zewnątrz → kwarantanna).
    pub async fn import_bytes(&self, bytes: &[u8]) -> Result<SkillImportResult, AppError> {
        let bundle = SkillBundle::from_bytes(bytes)
            .map_err(|e| AppError::invalid(format!("Paczka umiejętności: {e}")))?;
        let module = self.m()?;
        let report = module
            .import(&bundle, ImportOrigin::External)
            .await
            .map_err(skill_error)?;
        let all = module.list();
        let proposed = report
            .proposed
            .iter()
            .filter_map(|(id, v, _)| {
                all.iter()
                    .find(|r| &r.skill.id == id && r.skill.version.to_string() == *v)
                    .map(info)
            })
            .collect();
        Ok(SkillImportResult {
            proposed,
            skipped: report
                .skipped
                .into_iter()
                .map(|(id, why)| format!("{id}: {why}"))
                .collect(),
        })
    }
}

/// Zdarzenia `skills.*` (propozycja z pamięci, import, zatwierdzenie) → `SkillsChanged`.
pub async fn spawn_bridge(bus: Arc<dyn EventBus>, events: EventHub) {
    let Ok(mut stream) = bus.subscribe(EventFilter::prefix("skills.")).await else {
        return;
    };
    tokio::spawn(async move {
        while let Some(item) = stream.next().await {
            let BusItem::Event(event) = item else {
                continue;
            };
            let skill_id = event.payload["id"].as_str().map(str::to_owned);
            events.emit(AlfaEvent::SkillsChanged { skill_id });
        }
    });
}
