//! Z.8: składanie produkcyjnych sędziego, Routera i Brokera na atrapach wejścia/wyjścia.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use app_judge::NoteLog;
use compliance_contract::PathEnv;
use core_bus_contract::{Cost, SessionId};
use providers_fake::{FAKE_MODEL, FakeProvider, Script};
use router_contract::RouteKind;
use router_impl::RouterCore;
use safety_broker_contract::contract_tests::{self, PROFILE};
use safety_broker_contract::{
    ActionRequest, ApprovalId, Broker, Capability, CommandOrigin, Holder, ModelLocation,
};
use safety_broker_impl::audit::MemoryAudit;
use safety_broker_impl::{BrokerConfig, BrokerEngine, KeyMode};
use sessions_contract::{ModelUsage, NewSession, NewTurn, SessionCatalog, SessionHistory};
use sessions_fake::FakeSessions;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use watchdog_contract::ManualClock;

pub const ALLOW: &str = r#"{"verdict":"allow","reason":"właściciel zlecił pobranie"}"#;

pub struct Note {
    pub session: String,
    pub title: String,
    pub detail: String,
    pub warn: bool,
}

pub struct Rig {
    pub history: Arc<FakeSessions>,
    pub session: SessionId,
    pub broker: Arc<BrokerEngine>,
    pub audit: Arc<MemoryAudit>,
    pub clock: Arc<ManualClock>,
}

impl Rig {
    pub fn new(turns: Vec<NewTurn>) -> Self {
        let history = Arc::new(FakeSessions::new());
        let session = history.create_session(NewSession::default()).unwrap().id;
        let mut leaf = None;
        for turn in turns {
            leaf = Some(history.append_turn(&session, leaf, turn).unwrap().id);
        }
        let clock = Arc::new(ManualClock::new(1_000_000));
        let audit = Arc::new(MemoryAudit::default());
        let broker = Arc::new(
            BrokerEngine::new(
                BrokerConfig {
                    policy: contract_tests::test_policy(),
                    env: PathEnv::windows_profile(PROFILE),
                    key_mode: KeyMode::Deterministic(85),
                },
                clock.clone(),
                audit.clone(),
                Arc::new(platform_fake::FakeProcesses::default()),
            )
            .unwrap(),
        );
        Self {
            history,
            session,
            broker,
            audit,
            clock,
        }
    }

    pub fn holder(&self) -> Holder {
        Holder::agent(self.session.as_str(), "delta")
    }

    pub fn action(&self, location: Option<ModelLocation>) -> ActionRequest {
        let mut req = contract_tests::request(
            &self.holder(),
            Capability::NetEgress(contract_tests::host("api.nbp.pl")),
            CommandOrigin::UserText,
        );
        req.facts.tool = "net.fetch".into();
        req.facts.summary = Some("pobranie kursu EUR z api.nbp.pl".into());
        req.facts.model_location = location;
        req
    }

    pub async fn ask(&self, location: Option<ModelLocation>) -> ApprovalId {
        // Znak życia kierowcy, zanim narzędzie poprosi o zgodę.
        self.broker.judge_queue();
        contract_tests::needs_approval(self.broker.decide(self.action(location)).await).id
    }

    pub fn start(
        &self,
        local: Option<&Arc<RouterCore>>,
        hybrid: Option<&Arc<RouterCore>>,
    ) -> (
        providers_contract::CancellationToken,
        UnboundedReceiver<Note>,
    ) {
        let (tx, rx) = unbounded_channel();
        let log = Arc::new(NoteLog(move |session: &str, title, detail, warn| {
            let _ = tx.send(Note {
                session: session.into(),
                title,
                detail,
                warn,
            });
        }));
        let stop = app_judge::spawn(
            self.broker.clone(),
            local,
            hybrid,
            self.history.clone(),
            log,
        );
        (stop, rx)
    }
}

pub fn assistant(provider: &str) -> NewTurn {
    let mut turn = NewTurn::assistant("delta", "Pobieram kurs");
    turn.usage = Some(ModelUsage {
        provider: provider.into(),
        model: format!("{provider}:{FAKE_MODEL}"),
        cost: Cost::default(),
    });
    turn
}

pub fn router(id: &str, kind: RouteKind, script: Script) -> (Arc<RouterCore>, Arc<FakeProvider>) {
    let provider = Arc::new(FakeProvider::new(id).with_default_script(script));
    let router = Arc::new(RouterCore::default());
    router.register(provider.clone(), kind);
    (router, provider)
}

pub async fn note(rx: &mut UnboundedReceiver<Note>) -> Note {
    tokio::time::timeout(Duration::from_secs(35), rx.recv())
        .await
        .expect("kierowca powinien zakończyć ocenę")
        .expect("dziennik powinien dostać wpis")
}
