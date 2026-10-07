//! Zadanie dla mostu: rodzaj mostu, specyfikacja, katalog roboczy, budżet, pochodzenie uruchomienia.

use std::fmt;
use std::path::PathBuf;

use compliance_contract::{RouteId, SessionTag};
use core_bus_contract::SessionId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Most CLI (klasa C, PLAN §5.2). Kolejne (Grok Build, Kimi Code, `agy`) — po weryfikacji tras.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BridgeKind {
    /// Claude Code (`claude -p`, stream-json, `--permission-prompt-tool`).
    ClaudeCode,
    /// Codex CLI (`codex app-server`, JSON-RPC po stdio, zatwierdzenia serwera).
    Codex,
}

impl BridgeKind {
    /// Wszystkie mosty.
    pub const ALL: [BridgeKind; 2] = [BridgeKind::ClaudeCode, BridgeKind::Codex];

    /// Identyfikator trasy w rejestrze zgodności.
    pub fn route_id_str(self) -> &'static str {
        match self {
            BridgeKind::ClaudeCode => "claude-code-cli",
            BridgeKind::Codex => "codex-cli",
        }
    }

    /// Identyfikator trasy jako `RouteId` (stałe są poprawnym kebab-case, więc zawsze `Some`;
    /// wywołujący traktuje `None` jak trasę nieznaną = odmowa).
    pub fn route_id(self) -> Option<RouteId> {
        RouteId::new(self.route_id_str())
    }

    /// Nazwa programu CLI w PATH.
    pub fn program(self) -> &'static str {
        match self {
            BridgeKind::ClaudeCode => "claude",
            BridgeKind::Codex => "codex",
        }
    }
}

impl fmt::Display for BridgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.program())
    }
}

/// Identyfikator zadania (nadaje backend).
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct TaskId(pub String);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Skąd pochodzi żądanie uruchomienia mostu (PLAN §1.3 pkt 4; subscription-routes.md §2.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "origin", rename_all = "snake_case")]
pub enum LaunchOrigin {
    /// Bezpośrednie żądanie użytkownika (tekst, głos, paleta) — jedyne domyślnie dozwolone.
    UserRequest,
    /// Harmonogram — tylko z jawną, per-trasa zgodą użytkownika i dziennym limitem.
    Scheduled {
        /// Identyfikator harmonogramu.
        schedule_id: String,
    },
    /// Wyzwalacz (`triggers`) — mosty zabronione (subscription-routes.md §2.4).
    Trigger {
        /// Identyfikator wyzwalacza.
        trigger_id: String,
    },
    /// Ulepszacz (§12) — nigdy.
    Improver,
}

/// Tryb przygotowania katalogu roboczego (zawsze izolowany od katalogu użytkownika, §8.5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkdirMode {
    /// `git worktree` (repozytorium git); dla katalogu bez git — kopia.
    #[default]
    Worktree,
    /// Kopia katalogu.
    Copy,
}

/// Źródło katalogu roboczego zadania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkdirSpec {
    /// Katalog użytkownika (źródło) — most nigdy nie pracuje w nim bezpośrednio.
    pub source: PathBuf,
    /// Tryb.
    #[serde(default)]
    pub mode: WorkdirMode,
}

/// Budżet zadania.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskBudget {
    /// Maksymalna liczba tur agenta (Claude `--max-turns`).
    #[serde(default)]
    pub max_turns: Option<u32>,
    /// Limit czasu ściennego w ms (po nim zadanie jest anulowane z `BudgetExceeded`).
    #[serde(default)]
    pub wall_clock_ms: Option<u64>,
    /// Limit kosztu w mikro-USD (sprawdzany na zdarzeniach `Usage` z kosztem).
    #[serde(default)]
    pub max_cost_micro_usd: Option<u64>,
}

/// Odnośnik do sesji CLI (do wznowienia): identyfikator sesji/wątku CLI + katalog roboczy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionRef {
    /// Most.
    pub bridge: BridgeKind,
    /// Identyfikator sesji CLI (Claude `session_id`, Codex `threadId`) — nieprzezroczysty.
    pub id: String,
    /// Przygotowany katalog roboczy, w którym toczyła się sesja.
    pub workdir: PathBuf,
}

/// Specyfikacja zadania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskSpec {
    /// Most.
    pub bridge: BridgeKind,
    /// Polecenie.
    pub prompt: String,
    /// Katalog roboczy (źródło; backend przygotowuje worktree/kopię).
    pub workdir: WorkdirSpec,
    /// Narzędzia CLI dozwolone bez pytania (`--allowedTools`).
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Narzędzia CLI zabronione (`--disallowedTools`).
    #[serde(default)]
    pub disallowed_tools: Vec<String>,
    /// Budżet.
    #[serde(default)]
    pub budget: TaskBudget,
    /// Wznowienie sesji CLI (`--resume` / `thread/resume`); katalog roboczy z odnośnika.
    #[serde(default)]
    pub session: Option<SessionRef>,
    /// Sesja Alfy, z której pochodzi zadanie.
    pub alfa_session: SessionId,
    /// Tag prywatności sesji (polityka tras, `route_allowed`).
    #[serde(default)]
    pub privacy: SessionTag,
    /// Pochodzenie uruchomienia.
    pub origin: LaunchOrigin,
    /// Model CLI (opcjonalnie; domyślny model CLI użytkownika).
    #[serde(default)]
    pub model: Option<String>,
}

impl TaskSpec {
    /// Zadanie z żądania użytkownika z domyślnymi ustawieniami.
    pub fn user_request(
        bridge: BridgeKind,
        prompt: impl Into<String>,
        source: impl Into<PathBuf>,
        alfa_session: SessionId,
    ) -> Self {
        Self {
            bridge,
            prompt: prompt.into(),
            workdir: WorkdirSpec {
                source: source.into(),
                mode: WorkdirMode::Worktree,
            },
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            budget: TaskBudget::default(),
            session: None,
            alfa_session,
            privacy: SessionTag::Standard,
            origin: LaunchOrigin::UserRequest,
            model: None,
        }
    }
}

/// Uchwyt przyjętego zadania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskHandle {
    /// Zadanie.
    pub task: TaskId,
    /// Most.
    pub bridge: BridgeKind,
    /// Przygotowany (izolowany) katalog roboczy.
    pub workdir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_routes_and_programs() {
        assert_eq!(
            BridgeKind::ClaudeCode.route_id().unwrap().as_str(),
            "claude-code-cli"
        );
        assert_eq!(BridgeKind::Codex.route_id().unwrap().as_str(), "codex-cli");
        assert_eq!(BridgeKind::Codex.to_string(), "codex");
        let origin: LaunchOrigin =
            serde_json::from_value(serde_json::json!({"origin": "trigger", "trigger_id": "t"}))
                .unwrap();
        assert_eq!(
            origin,
            LaunchOrigin::Trigger {
                trigger_id: "t".into()
            }
        );
        let spec = TaskSpec::user_request(BridgeKind::ClaudeCode, "p", "/src", SessionId::new("s"));
        assert_eq!(spec.origin, LaunchOrigin::UserRequest);
        assert_eq!(spec.workdir.mode, WorkdirMode::Worktree);
        assert_eq!(TaskId("x".into()).to_string(), "x");
    }
}
