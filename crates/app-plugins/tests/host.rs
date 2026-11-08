//! Host operacji wtyczek w aplikacji: obrona w głąb (`Broker::verify` przed każdą operacją, bez
//! tokenu — odmowa), odczyt przez `FsPort`, zapis przez dziennik cofania (krok sesji do cofnięcia),
//! sieć tylko do hosta z tokenu `net.egress` przez klienta; pełna ścieżka wtyczka → host.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_plugins::{AlfaPluginHost, HostDeps};
use common::*;
use compliance_contract::{DenyLists, PathEnv};
use core_bus_contract::SessionId;
use plugin_runtime_contract::{HostError, HostOp, PluginHost, PluginLimits, samples};
use risk_classifier_contract::CommandOrigin;
use safety_broker_contract::{
    ActionRequest, ApprovalDecision, CapToken, Capability, DeclaredFacts, Holder, HostPattern,
    PathScope,
};
use safety_broker_fake::ScriptedDecision;
use serde_json::json;
use tools_common_contract::BrokerGate;
use undo_journal_contract::UndoJournal;

fn host(env: &Env) -> AlfaPluginHost {
    AlfaPluginHost::new(HostDeps {
        broker: env.broker.clone(),
        fs: env.fs.clone(),
        journal: env.journal.clone(),
        env: PathEnv::windows_profile(r"C:\Users\user"),
        deny: DenyLists::baseline(),
        net: env.net.clone(),
    })
}

async fn token(env: &Env, cap: Capability, tool: &str) -> CapToken {
    let gate = BrokerGate::new(env.broker.clone());
    let request = ActionRequest {
        holder: Holder::agent("s1", "delta"),
        capability: cap,
        facts: DeclaredFacts::new(tool),
        origin: CommandOrigin::Agent,
        ttl_ms: Some(60_000),
    };
    gate.authorize(request, &ctx()).await.unwrap().token
}

fn read(path: &str) -> HostOp {
    HostOp::FsReadText { path: path.into() }
}

#[tokio::test(flavor = "multi_thread")]
async fn host_verifies_every_token_itself() {
    let env = Env::new();
    let h = host(&env);
    let who = Holder::agent("s1", "delta");
    let denied = h.execute(&read(NOTE_A), None, &who).await.unwrap_err();
    assert!(matches!(denied, HostError::Denied(_)), "{denied:?}");
    let scope = PathScope::exact(NOTE_A, &PathEnv::new()).unwrap();
    let t = token(&env, Capability::FsRead(scope), "plugin.test.probe").await;
    let ok = h.execute(&read(NOTE_A), Some(&t), &who).await.unwrap();
    assert_eq!(ok, json!({"content": "notatka A"}));
    // Ten sam token na inny plik albo dla innej agentki — `verify` odmawia przed portem.
    let other = r"C:\Users\user\Documents\notes\b.txt";
    let e = h.execute(&read(other), Some(&t), &who).await.unwrap_err();
    assert!(matches!(e, HostError::Denied(_)), "{e:?}");
    let e = h
        .execute(&read(NOTE_A), Some(&t), &Holder::agent("s1", "gama"))
        .await
        .unwrap_err();
    assert!(matches!(e, HostError::Denied(_)), "{e:?}");
    let dotted = r"C:\Users\user\Documents\notes\..\notes\a.txt";
    let e = h.execute(&read(dotted), Some(&t), &who).await.unwrap_err();
    assert!(
        matches!(e, HostError::Denied(_) | HostError::BadArgs(_)),
        "{e:?}"
    );
    let log = HostOp::Log {
        message: "x".into(),
    };
    assert_eq!(h.execute(&log, None, &who).await.unwrap(), json!(null));
}

#[tokio::test(flavor = "multi_thread")]
async fn net_get_only_to_the_granted_host() {
    let env = Env::new();
    env.broker
        .script("plugin.test.probe", ScriptedDecision::Allow);
    let h = host(&env);
    let who = Holder::agent("s1", "delta");
    let cap = Capability::NetEgress(HostPattern::parse("api.example.com").unwrap());
    let t = token(&env, cap, "plugin.test.probe").await;
    let get = |url: &str| HostOp::NetGet { url: url.into() };
    let ok = h
        .execute(&get("https://api.example.com/kurs"), Some(&t), &who)
        .await
        .unwrap();
    assert_eq!(ok["status"], 200);
    let e = h
        .execute(&get("https://evil.example.org/x"), Some(&t), &who)
        .await
        .unwrap_err();
    assert!(matches!(e, HostError::Denied(_)), "{e:?}");
    assert_eq!(
        env.net.calls.lock().unwrap().clone(),
        vec!["https://api.example.com/kurs".to_owned()],
        "obcy host nie dotarł do klienta sieci"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn plugin_reads_and_writes_through_journal() {
    let env = Env::new();
    let app = env.app();
    let tree = |w: bool| {
        let s = PathScope::tree(NOTES, &PathEnv::new()).unwrap();
        if w {
            Capability::FsWrite(s)
        } else {
            Capability::FsRead(s)
        }
    };
    let mut probe = samples::word_count_tool();
    probe.name = "probe".into();
    probe.input_schema = json!({
        "type": "object",
        "properties": {"path": {"type": "string"}, "content": {"type": "string"}},
        "required": ["path"],
        "additionalProperties": false
    });
    probe.output_schema = json!({"type": "object"});
    let install = |id: &'static str, op: &'static str, cap: Capability| {
        let app = &app;
        let probe = probe.clone();
        async move {
            let wasm = component(&proxy_body(op));
            let mut m = samples::manifest(id, "1.0.0", &wasm);
            m.tools = vec![probe];
            m.capabilities = vec![cap];
            m.limits = PluginLimits::default();
            let card = app
                .propose(serde_json::to_value(m).unwrap(), &b64(&wasm))
                .await
                .unwrap();
            app.approve(id, "1.0.0", &card.review_hash).await.unwrap();
        }
    };
    install("czytnik", "fs.read-text", tree(false)).await;
    let tool = app.tools().into_iter().next().unwrap();
    let out = tool.call(json!({"path": NOTE_A}), &ctx()).await;
    assert!(out.is_ok(), "{}", out.text);
    assert_eq!(out.data, json!({"content": "notatka A"}));
    app.remove("czytnik").await.unwrap();

    install("pisarz", "fs.write-text", tree(true)).await;
    env.broker
        .script("plugin.pisarz.probe", ScriptedDecision::NeedsApproval);
    tokio::spawn(approver(env.broker.clone(), ApprovalDecision::Allow));
    let path = r"C:\Users\user\Documents\notes\b.txt";
    let tool = app.tools().into_iter().next().unwrap();
    let out = tool
        .call(json!({"path": path, "content": "nowa"}), &ctx())
        .await;
    assert!(out.is_ok(), "{}", out.text);
    use platform_contract::FsPort;
    assert_eq!(env.fs.read(std::path::Path::new(path)).unwrap(), b"nowa");
    let steps = env.journal.steps(&SessionId::new("s1"));
    assert_eq!(steps.len(), 1, "zapis wtyczki = krok dziennika cofania");
    assert!(steps[0].reversible);
    env.journal.undo(steps[0].step).unwrap();
    assert!(env.fs.read(std::path::Path::new(path)).is_err(), "cofnięte");
}
