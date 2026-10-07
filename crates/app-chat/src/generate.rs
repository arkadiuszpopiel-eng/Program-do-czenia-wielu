//! Wysłanie wiadomości: tura użytkownika (append-only) → `ModelProvider::stream` → `TextDelta`
//! renderowane przyrostowo w Rust → zapis tury agentki → koszt → oś czasu. Najwyżej jedna
//! generacja na sesję; nowa wiadomość w tej samej sesji najpierw kończy (anuluje) poprzednią.
//! Identyfikator tury agentki jest rezerwowany przy starcie (numery tur są kolejne, a wszystkie
//! zapisy historii sesji przechodzą przez blokadę sesji i czekają na zapis aktywnej generacji).

use std::sync::{Arc, Mutex};

use personas_contract::PersonaId;
use providers_contract::CancellationToken;
use sessions_contract::{SessionHistory, SessionId, TurnId};
use tokio::sync::watch;

use app_api::AppError;
use app_api::dto::{self, AgentState, AgentStatus, AlfaEvent, ModelProfile, TurnStatus};
use app_api::ids;

use crate::engine::{ChatEngine, GenHandle};
use crate::{agent, delegate, stream};

/// Gdzie zapisać turę agentki.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Dziecko tury (zwykła odpowiedź, „kontynuuj").
    Child(TurnId),
    /// Rodzeństwo tury (wariant „ponów").
    Sibling(TurnId),
}

/// Żądanie generacji.
#[derive(Debug, Clone)]
pub struct GenRequest {
    /// Sesja.
    pub session: SessionId,
    /// Miejsce tury agentki w drzewie.
    pub placement: Placement,
    /// Ostatnia tura historii przekazywanej modelowi.
    pub history_leaf: TurnId,
    /// Agentka odpowiadająca.
    pub agent: String,
    /// Profil modelu (`None` = domyślny sesji).
    pub profile: Option<ModelProfile>,
    /// Tura kontynuowana („Kontynuuj").
    pub continues: Option<TurnId>,
    /// Źródło polecenia (tekst / głos — fakty dla Brokera w przebiegu agentki).
    pub origin: risk_classifier_contract::CommandOrigin,
    /// Odbiorca tekstu odpowiedzi (rozmowa głosowa) — obok zdarzeń UI.
    pub tap: Option<tokio::sync::mpsc::UnboundedSender<app_api::ports::VoiceChunk>>,
}

impl GenRequest {
    /// Żądanie tekstowe (bez odbiorcy głosowego).
    pub fn text(
        session: SessionId,
        placement: Placement,
        history_leaf: TurnId,
        agent: String,
        profile: Option<ModelProfile>,
        continues: Option<TurnId>,
    ) -> Self {
        Self {
            session,
            placement,
            history_leaf,
            agent,
            profile,
            continues,
            origin: risk_classifier_contract::CommandOrigin::UserText,
            tap: None,
        }
    }
}

/// Rola w obsadzie w kolejności ważności (pierwsza obsadzona = rola tury).
const ROLE_PRIORITY: [&str; 9] = [
    "conductor",
    "coder",
    "operator",
    "researcher",
    "writer",
    "keeper",
    "critic",
    "thinker",
    "speaker",
];

impl ChatEngine {
    /// Główna rola agentki w obsadzie sesji (identyfikator UI).
    pub fn role_of(&self, session: &SessionId, agent: &str) -> Option<String> {
        let roles = self
            .inner
            .personas
            .cast(session)
            .roles_of(&PersonaId::new(agent));
        ROLE_PRIORITY
            .iter()
            .find(|r| roles.iter().any(|x| x.as_str() == **r))
            .map(|r| (*r).to_owned())
            .or_else(|| roles.iter().next().map(|r| r.as_str().to_owned()))
    }

    /// Agentki sesji ze stanem (mówi = aktywna generacja).
    pub fn agents_of(&self, session: &SessionId) -> Vec<AgentState> {
        let cast = self.inner.personas.cast(session);
        let speaking = self.generation(session).map(|g| g.agent);
        let working = self
            .rt()
            .runs
            .get(session)
            .map(|r| (r.agent.clone(), r.waiting()));
        self.inner
            .personas
            .personas()
            .into_iter()
            .map(|p| AgentState {
                id: p.id.as_str().to_owned(),
                role_ids: cast
                    .roles_of(&p.id)
                    .iter()
                    .map(|r| r.as_str().to_owned())
                    .collect(),
                status: match &working {
                    Some((a, true)) if a == p.id.as_str() => AgentStatus::WaitingApproval,
                    Some((a, false)) if a == p.id.as_str() => AgentStatus::Working,
                    _ if speaking.as_deref() == Some(p.id.as_str()) => AgentStatus::Speaking,
                    _ => AgentStatus::Idle,
                },
                activity: None,
            })
            .collect()
    }

    /// `AgentsChanged` dla sesji.
    pub fn announce_agents(&self, session: &SessionId) {
        self.emit(AlfaEvent::AgentsChanged {
            session_id: session.to_string(),
            agents: self.agents_of(session),
        });
    }

    /// Startuje generację. Wywołujący trzyma blokadę sesji i zakończył poprzednią generację.
    /// Skażona sesja (załączniki, wcześniejsze uruchomienia) najpierw trafia do Brokera — błąd
    /// Brokera = tura nie startuje (fail-closed).
    pub async fn start_generation(&self, req: GenRequest) -> Result<String, AppError> {
        self.sync_taint(&req.session).await?;
        let sessions = &self.inner.sessions;
        let reserved = TurnId(sessions.turn_count(&req.session)? + 1);
        let parent = match req.placement {
            Placement::Child(p) => Some(p),
            Placement::Sibling(t) => sessions.turn(&req.session, t)?.parent,
        };
        let id = ids::turn_dto(&req.session, reserved);
        let live = dto::Turn {
            id: id.clone(),
            session_id: req.session.to_string(),
            parent_id: parent.map(|p| ids::turn_dto(&req.session, p)),
            author: req.agent.clone(),
            role_id: self.role_of(&req.session, &req.agent),
            created_at: dto::iso(chrono::Utc::now()),
            status: TurnStatus::Streaming,
            text: String::new(),
            blocks: Vec::new(),
            thinking: None,
            tools: Vec::new(),
            approval: None,
            usage: None,
            error: None,
            continues: req.continues.map(|c| ids::turn_dto(&req.session, c)),
            addressed_to: None,
            truncated: false,
            heard_prefix: None,
            attachments: Vec::new(),
        };
        let (done_tx, done_rx) = watch::channel(false);
        let handle = GenHandle {
            turn: reserved,
            agent: req.agent.clone(),
            cancel: CancellationToken::new(),
            done: done_rx,
            live: Arc::new(Mutex::new(live.clone())),
        };
        self.rt().gens.insert(req.session.clone(), handle.clone());
        self.emit(AlfaEvent::TurnAppended {
            session_id: req.session.to_string(),
            turn: Box::new(live),
        });
        self.announce_agents(&req.session);
        self.announce_session(&req.session).await;
        let core = self.clone();
        let delegation = self.delegation(&req);
        let setup = match (req.continues, &delegation) {
            (None, None) => self.agent_setup(&req.session, &req.agent),
            _ => None,
        };
        tokio::spawn(async move {
            let outcome = match (delegation, setup) {
                (Some(d), _) => delegate::run(&core, &req, &handle, d).await,
                (None, Some(setup)) => agent::run(&core, &req, &handle, setup).await,
                (None, None) => stream::generate(&core, &req, &handle).await,
            };
            if let Some(tap) = &req.tap {
                let last = match &outcome.error {
                    Some(e) => app_api::ports::VoiceChunk::Failed(e.message.clone()),
                    None => app_api::ports::VoiceChunk::Done,
                };
                let _ = tap.send(last);
            }
            core.finish_generation(&req, &handle, outcome).await;
            let _ = done_tx.send(true);
        });
        Ok(id)
    }
}
