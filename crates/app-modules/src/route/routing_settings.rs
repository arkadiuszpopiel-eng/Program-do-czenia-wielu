//! Ustawienia doboru modelu według trudności (Ustawienia → Dobór modelu): plik
//! `model-routing.json` w katalogu konfiguracji (bez sekretów), czytany ponownie tylko po zmianie
//! i nakładany na wszystkie rdzenie Routera; zmiana działa od następnej odpowiedzi. Tu też
//! przekład ustawień i modeli na typy interfejsu oraz podgląd decyzji dla przykładowego zadania.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

use app_api::dto::{
    ModelRoutingSettings, RoutingLevel, RoutingModel, RoutingPreview, RoutingRung, RoutingStrategy,
};
use providers_contract::{ChatRequest, Difficulty, Message, Strategy};
use router_contract::{
    Candidate, Constraints, DIFFICULTY_CLASSES, DifficultyPolicy, RouteKind, Rung, TaskClass,
};
use router_impl::RouterCore;

/// Nazwa pliku ustawień doboru w katalogu konfiguracji.
pub const FILE: &str = "model-routing.json";

/// Odczyt; brak pliku albo zły format — ustawienia domyślne (błąd formatu w dzienniku).
pub fn load(path: &Path) -> DifficultyPolicy {
    let Ok(text) = std::fs::read_to_string(path) else {
        return DifficultyPolicy::default();
    };
    serde_json::from_str(&text).unwrap_or_else(|e| {
        tracing::warn!(plik = %path.display(), "ustawienia doboru modelu nieczytelne: {e}");
        DifficultyPolicy::default()
    })
}

/// Zapis przez plik tymczasowy i zamianę nazwy.
pub fn save(path: &Path, policy: &DifficultyPolicy) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(policy).map_err(std::io::Error::other)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Plik ustawień z pamięcią czasu modyfikacji.
pub(crate) struct PolicyFile {
    path: PathBuf,
    seen: Mutex<Option<Option<SystemTime>>>,
}

impl PolicyFile {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            seen: Mutex::new(None),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Nowe ustawienia, gdy plik zmienił się od ostatniego odczytu (pierwszy odczyt — zawsze).
    pub fn changed(&self) -> Option<DifficultyPolicy> {
        let modified = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok();
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        if *seen == Some(modified) {
            return None;
        }
        *seen = Some(modified);
        Some(load(&self.path))
    }

    /// Wymusza ponowny odczyt przy następnym sprawdzeniu.
    pub fn forget(&self) {
        *self.seen.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

fn level_dto(d: Difficulty) -> RoutingLevel {
    match d {
        Difficulty::Easy => RoutingLevel::Easy,
        Difficulty::Medium => RoutingLevel::Medium,
        Difficulty::Hard => RoutingLevel::Hard,
    }
}

fn level_of(d: RoutingLevel) -> Difficulty {
    match d {
        RoutingLevel::Easy => Difficulty::Easy,
        RoutingLevel::Medium => Difficulty::Medium,
        RoutingLevel::Hard => Difficulty::Hard,
    }
}

fn rung_dto(r: Rung) -> RoutingRung {
    match r {
        Rung::Light => RoutingRung::Light,
        Rung::Standard => RoutingRung::Standard,
        Rung::Strong => RoutingRung::Strong,
    }
}

fn rung_of(r: RoutingRung) -> Rung {
    match r {
        RoutingRung::Light => Rung::Light,
        RoutingRung::Standard => Rung::Standard,
        RoutingRung::Strong => Rung::Strong,
    }
}

/// Ustawienia → typ interfejsu.
pub fn settings_dto(p: &DifficultyPolicy) -> ModelRoutingSettings {
    ModelRoutingSettings {
        enabled: p.enabled,
        strategy: match p.strategy {
            Strategy::Economy => RoutingStrategy::Economy,
            Strategy::Balanced => RoutingStrategy::Balanced,
            Strategy::Quality => RoutingStrategy::Quality,
        },
        escalate: p.escalate,
        review: level_dto(p.review),
        local_first: p.local_first,
        rungs: p
            .rungs
            .iter()
            .map(|(k, v)| (k.clone(), rung_dto(*v)))
            .collect(),
        reorder_queues: p.reorder_queues,
    }
}

/// Typ interfejsu → ustawienia; `Err` — kandydat w zapisie innym niż `dostawca:model`.
pub fn policy_of(s: &ModelRoutingSettings) -> Result<DifficultyPolicy, String> {
    let mut rungs = std::collections::BTreeMap::new();
    for (key, rung) in &s.rungs {
        let c = Candidate::parse(key.trim())
            .ok_or_else(|| format!("„{key}” nie jest modelem w zapisie dostawca:model."))?;
        rungs.insert(c.qualified(), rung_of(*rung));
    }
    Ok(DifficultyPolicy {
        enabled: s.enabled,
        strategy: match s.strategy {
            RoutingStrategy::Economy => Strategy::Economy,
            RoutingStrategy::Balanced => Strategy::Balanced,
            RoutingStrategy::Quality => Strategy::Quality,
        },
        escalate: s.escalate,
        review: level_of(s.review),
        local_first: s.local_first,
        rungs,
        reorder_queues: s.reorder_queues,
    })
}

/// Model Routera z szczeblem; `name` — nazwa dostawcy do interfejsu.
pub(crate) fn model_of(core: &RouterCore, c: &Candidate, name: String) -> RoutingModel {
    RoutingModel {
        candidate: c.qualified(),
        provider_name: name,
        model: c.model.clone(),
        local: core
            .registered(&c.provider)
            .is_some_and(|r| r.kind == RouteKind::Local),
        rung: rung_dto(core.rung_of(c)),
        automatic: rung_dto(core.automatic_rung(c)),
    }
}

/// Kandydaci klas z doborem w rdzeniach (bez powtórzeń, w kolejności pierwszego wystąpienia).
pub(crate) fn candidates(cores: &[&RouterCore]) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    for core in cores {
        let policy = router_contract::Router::policy(*core);
        for class in DIFFICULTY_CLASSES {
            for c in policy.candidates(class) {
                if !out.contains(c) {
                    out.push(c.clone());
                }
            }
        }
    }
    out
}

/// Podgląd decyzji dla przykładowego tekstu w klasie zadań.
pub(crate) fn preview(
    core: &RouterCore,
    class: TaskClass,
    text: &str,
    name: impl Fn(&Candidate) -> String,
) -> RoutingPreview {
    let req = ChatRequest::new(router_contract::AUTO_MODEL, vec![Message::user_text(text)]);
    let constraints = Constraints::from_request(class, &req);
    let note = core.judge(class, &constraints, Some(&req));
    let order = core
        .decide(class, &constraints, Some(&req))
        .map(|d| d.targets().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    RoutingPreview {
        difficulty: note.as_ref().map(|n| level_dto(n.difficulty)),
        score: note.as_ref().and_then(|n| n.score),
        reasons: note.as_ref().map(|n| n.reasons.clone()).unwrap_or_default(),
        start: note.as_ref().map(|n| rung_dto(n.start)),
        kept_queue: note.as_ref().is_some_and(|n| n.kept_queue),
        order: order.iter().map(|c| model_of(core, c, name(c))).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_round_trip_defaults_and_change_detection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        assert_eq!(load(&path), DifficultyPolicy::default());
        let file = PolicyFile::new(path.clone());
        assert_eq!(file.changed(), Some(DifficultyPolicy::default()));
        assert_eq!(file.changed(), None, "bez zmiany — bez ponownego odczytu");
        let mut p = DifficultyPolicy::default();
        p.strategy = Strategy::Quality;
        p.rungs.insert("openai:gpt-6-luna".into(), Rung::Strong);
        save(&path, &p).unwrap();
        file.forget();
        assert_eq!(file.changed(), Some(p.clone()));
        std::fs::write(&path, "{zły json").unwrap();
        assert_eq!(load(&path), DifficultyPolicy::default());
    }

    #[test]
    fn settings_convert_both_ways_and_reject_bad_keys() {
        let mut p = DifficultyPolicy::default();
        p.review = Difficulty::Medium;
        p.rungs.insert("local:qwen3-27b".into(), Rung::Light);
        let dto = settings_dto(&p);
        assert_eq!(dto.review, RoutingLevel::Medium);
        assert_eq!(policy_of(&dto).unwrap(), p);
        let mut bad = dto;
        bad.rungs
            .insert("bez-dwukropka".into(), RoutingRung::Strong);
        assert!(policy_of(&bad).unwrap_err().contains("dostawca:model"));
    }
}
