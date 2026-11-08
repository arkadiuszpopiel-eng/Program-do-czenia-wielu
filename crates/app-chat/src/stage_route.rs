//! Dobór dostawcy każdego etapu według jego roli i opcjonalnego przypiętego modelu.
use crate::engine::ChatEngine;
use crate::generate::GenRequest;
use agent_runtime_contract::RunSpec;
use app_api::ports::{BrainChoice, BrainRequest};
use providers_contract::{ChatRequest, Message};
use sessions_contract::SessionCatalog;

pub(crate) async fn choose(
    core: &ChatEngine,
    req: &GenRequest,
    spec: &RunSpec,
) -> Result<BrainChoice, String> {
    let mut messages = spec.history.clone();
    messages.push(Message::user_text(&spec.goal));
    let mut chat = ChatRequest::new(&spec.model, messages);
    chat.meta.session = Some(req.session.to_string());
    if let Some(stack) = &core.inner.agents {
        chat.tools = stack.tools.model_specs_for(&spec.roles, &spec.goal);
    }
    let privacy = core
        .inner
        .sessions
        .session(&req.session)
        .map(|s| s.privacy)
        .unwrap_or_default();
    chat.meta.privacy.tag = match privacy {
        sessions_contract::PrivacyTag::Normal => providers_contract::PrivacyTag::Normal,
        _ => providers_contract::PrivacyTag::Private,
    };
    core.inner
        .brain
        .choose(&BrainRequest {
            session: req.session.clone(),
            agent: spec.agent.to_string(),
            profile: req.profile,
            privacy,
            chat: Some(chat),
            task: spec.roles.first().map(|r| r.model_policy.clone()),
        })
        .await
        .map_err(|e| format!("Nie można dobrać modelu etapu: {e}"))
}
