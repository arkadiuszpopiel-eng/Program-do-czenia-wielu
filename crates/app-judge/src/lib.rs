//! Model-sędzia trybu „Automatycznie” w aplikacji (plan 50 §4.3, Z.4): kontekst rozmowy z historii sesji,
//! sędzia lokalny (Router tylko lokalny) i hybrydowy (Router z kluczami, szczebel sprawdzania), pętla
//! kierowcy pytająca Brokera o prośby co sekundę.

mod context;
mod note;

use std::sync::Arc;

use approval_judge_impl::{JudgeConfig, JudgeDriver, Judges, ModelJudge};
use providers_contract::{CancellationToken, ModelProvider};
use router_contract::TaskClass;
use router_impl::{RoutedProvider, RouterCore};
use safety_broker_contract::Broker;
use sessions_contract::SessionHistory;

pub use context::{SessionContext, context_from_turns};
pub use note::{NoteLog, describe};

fn judge_on(core: &Arc<RouterCore>) -> Arc<dyn approval_judge_contract::ApprovalJudge> {
    let provider: Arc<dyn ModelProvider> =
        Arc::new(RoutedProvider::new(core.clone(), TaskClass::Conversation));
    Arc::new(ModelJudge::new(provider, JudgeConfig::default()))
}

/// Uruchamia kierowcę sędziego. `local` — rdzeń Routera bez chmury (sędzia dla przebiegów lokalnych),
/// `hybrid` — rdzeń z kluczami API (przebiegi w chmurze), `log` — dziennik werdyktów. Zwraca token zatrzymania.
pub fn spawn(
    broker: Arc<dyn Broker>,
    local: Option<&Arc<RouterCore>>,
    hybrid: Option<&Arc<RouterCore>>,
    history: Arc<dyn SessionHistory>,
    log: Arc<dyn approval_judge_contract::JudgeLog>,
) -> CancellationToken {
    let judges = Judges {
        local: local.map(judge_on),
        any: hybrid.map(judge_on),
    };
    let driver = JudgeDriver::with_log(broker, judges, Arc::new(SessionContext::new(history)), log);
    let cancel = CancellationToken::new();
    tokio::spawn(driver.run(cancel.clone()));
    cancel
}
