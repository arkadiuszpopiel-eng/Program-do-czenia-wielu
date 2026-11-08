//! Kontekst sędziego z historii sesji (aktywna gałąź): cel = pierwsza wiadomość właściciela, zwięzły zapis
//! bez myślenia modelu, wyniki narzędzi jako treść obca, granice z wiadomości właściciela, zadanie = ostatnia
//! wiadomość właściciela, pochodzenie modelu z ostatniej odpowiedzi (nieznane → tylko sędzia lokalny).

use std::sync::Arc;

use approval_judge_contract::{JudgeContext, JudgeContextSource, JudgeTurn};
use core_bus_contract::SessionId;
use safety_broker_contract::ModelLocation;
use sessions_contract::{Author, Block, Role, SessionHistory, Turn};

/// Ile ostatnich tur trafia do zapisu (resztę i tak przycina budżet promptu).
const MAX_TURNS: usize = 40;

/// Słowa granic właściciela („nie wypychaj”, „nie usuwaj”…) — zdanie z nimi trafia do `limits`.
const BOUNDARY_VERBS: [&str; 14] = [
    "wypych", "push", "usuw", "kasuj", "wysył", "publik", "instal", "commit", "zmieniaj",
    "dotykaj", "nadpis", "formatuj", "płać", "kupuj",
];

fn limits_in(text: &str) -> Vec<String> {
    text.split(['.', '!', '\n', ';'])
        .map(str::trim)
        .filter(|s| {
            let low = s.to_lowercase();
            (low.starts_with("nie ")
                || low.contains(" nie ")
                || low.contains("never")
                || low.contains("don't"))
                && BOUNDARY_VERBS.iter().any(|v| low.contains(v))
        })
        .map(str::to_owned)
        .collect()
}

fn agent_step(turn: &Turn) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if !turn.content.text.trim().is_empty() {
        parts.push(turn.content.text.trim().to_owned());
    }
    for b in &turn.content.blocks {
        if let Block::ToolUse { name, input, .. } = b {
            parts.push(format!("wywołanie {name}: {input}"));
        }
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn tool_results(turn: &Turn, out: &mut Vec<JudgeTurn>) {
    let tool = match &turn.author {
        Author::Tool { name } => name.clone(),
        _ => "narzędzie".to_owned(),
    };
    for b in &turn.content.blocks {
        if let Block::ToolResult { content, .. } = b {
            out.push(JudgeTurn::ToolResult {
                tool: tool.clone(),
                text: content.clone(),
            });
        }
    }
    if turn.role == Role::Tool && !turn.content.text.trim().is_empty() {
        out.push(JudgeTurn::ToolResult {
            tool,
            text: turn.content.text.clone(),
        });
    }
}

fn is_local(turn: &Turn) -> Option<bool> {
    let usage = turn.usage.as_ref()?;
    Some(
        ModelLocation::of_model(&usage.model) == ModelLocation::Local
            || ModelLocation::of_model(&usage.provider) == ModelLocation::Local,
    )
}

/// Kontekst z liniowej historii (od korzenia).
pub fn context_from_turns(turns: &[Turn]) -> JudgeContext {
    let owner = |t: &&Turn| t.role == Role::User && matches!(t.author, Author::User);
    let goal = turns
        .iter()
        .find(owner)
        .map(|t| t.content.text.clone())
        .unwrap_or_default();
    let task = turns
        .iter()
        .rev()
        .find(owner)
        .map(|t| format!("tura-{}", t.id.0));
    let limits = turns
        .iter()
        .filter(owner)
        .flat_map(|t| limits_in(&t.content.text))
        .collect();
    // Liczy się najnowsza odpowiedź. Brak jej pochodzenia nie dziedziczy informacji ze starszej tury;
    // zostaje sędzia lokalny (zapis nie wychodzi z komputera).
    let local_only = turns
        .iter()
        .rev()
        .find(|t| t.role == Role::Assistant)
        .and_then(is_local)
        .unwrap_or(true);
    let mut transcript = Vec::new();
    let start = turns.len().saturating_sub(MAX_TURNS);
    for t in &turns[start..] {
        match t.role {
            Role::User if matches!(t.author, Author::User) => transcript.push(JudgeTurn::Owner {
                text: t.content.text.clone(),
            }),
            Role::Assistant => {
                if let Some(text) = agent_step(t) {
                    transcript.push(JudgeTurn::AgentStep { text });
                }
                tool_results(t, &mut transcript);
            }
            // Treść spoza właściciela (narzędzia, przekazania, system) — zawsze jako dane obce.
            _ => tool_results(t, &mut transcript),
        }
    }
    JudgeContext {
        goal,
        transcript,
        limits,
        task,
        local_only,
    }
}

/// Źródło kontekstu na historii sesji.
pub struct SessionContext {
    history: Arc<dyn SessionHistory>,
}

impl SessionContext {
    /// Nowe źródło.
    pub fn new(history: Arc<dyn SessionHistory>) -> Self {
        Self { history }
    }
}

impl JudgeContextSource for SessionContext {
    fn context(&self, session: &str, _agent: Option<&str>) -> Option<JudgeContext> {
        let id = SessionId::new(session);
        let leaf = self.history.active_leaf(&id).ok()??;
        let turns = self.history.branch_projection(&id, leaf).ok()?;
        Some(context_from_turns(&turns))
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
