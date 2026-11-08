//! Wyszukiwanie umiejętności pasującej do zadania: deterministyczne (słowa po `fold`,
//! przycięte do rdzenia — odmiana PL, wagi pól, próg) wyłącznie wśród zainstalowanych
//! i uruchamialnych przez rolę wywołującej; opcjonalny port LLM ([`SkillRanker`]) tylko
//! **przestawia** kandydatów deterministycznych (nie może wprowadzić innych), a jego błąd
//! = kolejność deterministyczna.

use std::collections::BTreeSet;

use async_trait::async_trait;
use personas_contract::{Role, fold};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::{SkillId, SkillRecord, SkillState};
use crate::run::runnable_by;
use tools_common_contract::ToolManifest;

/// Długość rdzenia słowa (odmiana: „pobrane”/„pobranych” → „pobra”).
const STEM: usize = 5;
/// Próg dopasowania.
pub const MIN_SCORE: f32 = 0.2;
/// Ilu kandydatów dostaje model do przestawienia.
pub const RERANK_TOP: usize = 8;

const STOP: [&str; 24] = [
    "i", "w", "z", "na", "do", "sie", "oraz", "to", "ze", "o", "po", "dla", "od", "mi", "mnie",
    "prosze", "jak", "a", "the", "of", "and", "for", "my", "moje",
];

/// Rdzenie słów tekstu (bez słów pomocniczych).
pub fn stems(text: &str) -> BTreeSet<String> {
    fold(text)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 2 && !STOP.contains(w))
        .map(|w| w.chars().take(STEM).collect())
        .collect()
}

/// Dopasowanie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SkillMatch {
    /// Umiejętność.
    pub id: SkillId,
    /// Wersja.
    pub version: String,
    /// Nazwa.
    pub name: String,
    /// Wynik 0–1.
    pub score: f32,
    /// Dopasowane rdzenie (uzasadnienie dla UI).
    pub matched: Vec<String>,
}

/// Wynik dopasowania przepisu do zadania (0–1): pola z wagami — nazwa i słowa kluczowe 3,
/// opis 2, szablon i kroki 1.
pub fn score(record: &SkillRecord, task: &str) -> (f32, Vec<String>) {
    let q = stems(task);
    if q.is_empty() {
        return (0.0, Vec::new());
    }
    let s = &record.skill;
    let fields = [
        (
            stems(&format!("{} {}", s.name, s.keywords.join(" "))),
            3.0f32,
        ),
        (stems(&s.description), 2.0),
        (stems(&format!("{} {}", s.prompt, s.steps.join(" "))), 1.0),
    ];
    let mut total = 0.0f32;
    let mut matched = Vec::new();
    for w in &q {
        let best = fields
            .iter()
            .filter(|(f, _)| f.contains(w))
            .map(|(_, weight)| *weight)
            .fold(0.0f32, f32::max);
        if best > 0.0 {
            matched.push(w.clone());
        }
        total += best;
    }
    let n = q.len() as f32;
    ((total / (3.0 * n)).clamp(0.0, 1.0), matched)
}

/// Deterministyczne wyszukiwanie: zainstalowane, uruchamialne przez role wywołującej, wynik ≥
/// progu; malejąco po wyniku, remis — po identyfikatorze.
pub fn search(
    records: &[SkillRecord],
    catalog: &[ToolManifest],
    caller_roles: &[Role],
    task: &str,
    limit: usize,
) -> Vec<SkillMatch> {
    let mut out: Vec<SkillMatch> = records
        .iter()
        .filter(|r| r.state == SkillState::Installed)
        .filter(|r| runnable_by(&r.skill, catalog, caller_roles).is_ok())
        .filter_map(|r| {
            let (score, matched) = score(r, task);
            (score >= MIN_SCORE).then(|| SkillMatch {
                id: r.skill.id.clone(),
                version: r.skill.version.to_string(),
                name: r.skill.name.clone(),
                score,
                matched,
            })
        })
        .collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    out.truncate(limit);
    out
}

/// Port do modelu: przestawia kandydatów (zwraca identyfikatory z oceną 0–1).
#[async_trait]
pub trait SkillRanker: Send + Sync {
    /// Ocena kandydatów dla zadania.
    async fn rank(
        &self,
        task: &str,
        candidates: &[SkillMatch],
    ) -> Result<Vec<(SkillId, f32)>, String>;
}

/// Wyszukiwanie z modelem: deterministyczni kandydaci (≤ [`RERANK_TOP`]) przestawieni przez
/// model; identyfikatory spoza kandydatów są ignorowane, oceny przycinane do 0–1, kandydaci
/// pominięci przez model zostają za ocenionymi; błąd modelu = kolejność deterministyczna.
pub async fn rerank(
    ranker: &dyn SkillRanker,
    task: &str,
    candidates: Vec<SkillMatch>,
) -> Vec<SkillMatch> {
    let top: Vec<SkillMatch> = candidates.into_iter().take(RERANK_TOP).collect();
    let Ok(ranked) = ranker.rank(task, &top).await else {
        return top;
    };
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(top.len());
    for (id, s) in ranked {
        if let Some(m) = top.iter().find(|m| m.id == id)
            && seen.insert(id)
        {
            let mut m = m.clone();
            m.score = if s.is_finite() {
                s.clamp(0.0, 1.0)
            } else {
                0.0
            };
            out.push(m);
        }
    }
    out.extend(top.into_iter().filter(|m| !seen.contains(&m.id)));
    out
}
