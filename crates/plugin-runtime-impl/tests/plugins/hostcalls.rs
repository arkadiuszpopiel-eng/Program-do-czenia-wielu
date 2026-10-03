//! Operacje hosta: jedyne wyjście z piaskownicy. Zdolność spoza manifestu, zakres szerszy
//! niż zadeklarowany, przejście `..`, blokady Jądra, odmowa i brak decyzji właściciela,
//! nieznane operacje, pola dodatkowe, limity — zawsze odmowa, nic nie wykonane, nic nie
//! wycieka. Pozytywnie: odczyt w zakresie z tokenem Brokera, `log`, zgoda właściciela.

use std::sync::Arc;
use std::time::Duration;

use plugin_runtime_contract::PluginLimits;
use risk_classifier_contract::KernelRule;
use safety_broker_contract::{ApprovalDecision, Capability};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::{Value, json};
use tools_common_contract::ToolOutcome;

use crate::common::*;

pub(crate) const NOTES: &str = r"C:\Users\user\Documents\notes";
pub(crate) const NOTE_A: &str = r"C:\Users\user\Documents\notes\a.txt";

/// Ciało: operacja hosta o stałych argumentach (także niepoprawnych).
fn fixed_call_body(op: &str, args: &[u8]) -> String {
    format!(
        r#"(data (i32.const 256) "{}") (data (i32.const 512) "{}")
    (func (export "invoke") (param i32 i32 i32 i32) (result i32)
      (local $r i32)
      (local.set $r (call $realloc (i32.const 0) (i32.const 0) (i32.const 4) (i32.const 12)))
      (call $call (i32.const 256) (i32.const {}) (i32.const 512) (i32.const {}) (local.get $r))
      (local.get $r))"#,
        wat_str(op.as_bytes()),
        wat_str(args),
        op.len(),
        args.len()
    )
}

struct Case {
    name: &'static str,
    wasm: Vec<u8>,
    caps: Vec<Capability>,
    limits: PluginLimits,
    args: Value,
    script: Option<ScriptedDecision>,
    expect: &'static str,
}

fn case(
    name: &'static str,
    op: &str,
    caps: Vec<Capability>,
    args: Value,
    expect: &'static str,
) -> Case {
    Case {
        name,
        wasm: component(&proxy_body(op)),
        caps,
        limits: PluginLimits::default(),
        args,
        script: None,
        expect,
    }
}

fn denial_cases() -> Vec<Case> {
    let notes = || vec![fs_read_tree(NOTES)];
    let not_declared = "nie zadeklarowała";
    let bad = "niepoprawne argumenty";
    let broker = "odmowa Brokera";
    let mut scripted = case(
        "Broker: blokada skryptowana",
        "fs.read-text",
        notes(),
        json!({"path": NOTE_A}),
        broker,
    );
    scripted.script = Some(ScriptedDecision::Deny(KernelRule::CredentialDenylist));
    let mut no_answer = case(
        "zgoda bez odpowiedzi (limit czasu)",
        "fs.read-text",
        notes(),
        json!({"path": NOTE_A}),
        broker,
    );
    no_answer.script = Some(ScriptedDecision::NeedsApproval);
    let mut many = case(
        "limit operacji hosta",
        "log",
        Vec::new(),
        json!({}),
        "operacji hosta",
    );
    many.wasm = component(
        r#"(data (i32.const 256) "log") (data (i32.const 300) "{\"message\":\"x\"}")
    (func (export "invoke") (param i32 i32 i32 i32) (result i32)
      (local $r i32)
      (local.set $r (call $realloc (i32.const 0) (i32.const 0) (i32.const 4) (i32.const 12)))
      (loop $l
        (call $call (i32.const 256) (i32.const 3) (i32.const 300) (i32.const 15) (local.get $r))
        (br $l))
      (local.get $r))"#,
    );
    let mut huge = case(
        "argumenty ponad limit",
        "fs.read-text",
        notes(),
        json!({"path": "C:\\".to_owned() + &"a\\".repeat(100)}),
        "przekraczają limit",
    );
    huge.limits.max_output_bytes = 64;
    let fixed = |name, op: &str, args: &[u8], expect| Case {
        wasm: component(&fixed_call_body(op, args)),
        ..case(name, op, notes(), json!({}), expect)
    };
    vec![
        case(
            "odczyt bez zdolności",
            "fs.read-text",
            Vec::new(),
            json!({"path": NOTE_A}),
            not_declared,
        ),
        case(
            "odczyt poza zakresem (.ssh)",
            "fs.read-text",
            notes(),
            json!({"path": r"C:\Users\user\.ssh\id_rsa"}),
            not_declared,
        ),
        case(
            "przejście ..",
            "fs.read-text",
            notes(),
            json!({"path": r"C:\Users\user\Documents\notes\..\..\.ssh\id_rsa"}),
            not_declared,
        ),
        case(
            "ścieżka względna",
            "fs.read-text",
            notes(),
            json!({"path": r"..\..\.ssh\id_rsa"}),
            bad,
        ),
        case(
            "ścieżka urządzenia",
            "fs.read-text",
            notes(),
            json!({"path": r"\\.\PhysicalDrive0"}),
            "nie zadeklarowała|niepoprawne argumenty",
        ),
        case(
            "zmienna środowiskowa w ścieżce",
            "fs.read-text",
            notes(),
            json!({"path": r"%USERPROFILE%\.ssh\id_rsa"}),
            "niepoprawne argumenty|nie zadeklarowała",
        ),
        case(
            "zapis bez zdolności zapisu",
            "fs.write-text",
            notes(),
            json!({"path": NOTE_A, "content": "x"}),
            not_declared,
        ),
        case(
            "poświadczenia CLI (Jądro)",
            "fs.read-text",
            vec![fs_read_tree(r"C:\Users\user\.claude")],
            json!({"path": r"C:\Users\user\.claude\.credentials.json"}),
            broker,
        ),
        scripted,
        no_answer,
        case(
            "nieznana operacja shell.exec",
            "shell.exec",
            notes(),
            json!({"cmd": "calc"}),
            "nieznana operacja",
        ),
        case(
            "nieznana operacja secrets.read",
            "secrets.read",
            notes(),
            json!({"id": "anthropic"}),
            "nieznana operacja",
        ),
        case(
            "pole dodatkowe",
            "fs.read-text",
            notes(),
            json!({"path": NOTE_A, "mode": "admin"}),
            bad,
        ),
        fixed("argumenty nie-JSON", "fs.read-text", b"{path: C:", bad),
        fixed(
            "argumenty z bajtami spoza UTF-8",
            "fs.read-text",
            b"\xff\xfe",
            "trap|niepoprawne argumenty",
        ),
        case(
            "net.get bez https",
            "net.get",
            vec![egress("api.example.com")],
            json!({"url": "http://api.example.com/x"}),
            bad,
        ),
        case(
            "net.get do niezadeklarowanego hosta",
            "net.get",
            vec![egress("api.example.com")],
            json!({"url": "https://evil.example.org/x"}),
            not_declared,
        ),
        case(
            "net.get z danymi logowania w adresie",
            "net.get",
            vec![egress("api.example.com")],
            json!({"url": "https://api.example.com@evil.org/x"}),
            bad,
        ),
        many,
        huge,
    ]
}

pub(crate) async fn approver(broker: Arc<FakeBroker>, decision: ApprovalDecision) {
    for _ in 0..200 {
        if broker.auto_approve(decision.clone()).await > 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn assert_no_leak(name: &str, out: &ToolOutcome, h: &Harness) {
    for secret in ["SEKRET-KLUCZ", "TOKEN-CLI"] {
        assert!(
            !out.text.contains(secret) && !out.data.to_string().contains(secret),
            "{name}: wyciek"
        );
    }
    assert!(
        h.host.calls().iter().all(|c| c.op == "log"),
        "{name}: host wykonał operację {:?}",
        h.host.calls()
    );
    assert_eq!(
        h.host.file(NOTE_A).as_deref(),
        Some("notatka A"),
        "{name}: plik zmieniony"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn host_calls_outside_grants_are_denied() {
    let cases = denial_cases();
    for c in &cases {
        let h = harness();
        if let Some(s) = &c.script {
            h.broker.script("plugin.napastniczka.probe", *s);
        }
        let out = h
            .probe(
                "napastniczka",
                c.wasm.clone(),
                c.caps.clone(),
                c.limits,
                c.args.clone(),
            )
            .await;
        assert!(!out.is_ok(), "{}: {out:?}", c.name);
        let ok = c
            .expect
            .split('|')
            .any(|e| out.text.contains(e) || out.data["error"] == e);
        assert!(ok, "{}: {}", c.name, out.text);
        assert_no_leak(c.name, &out, &h);
    }
    eprintln!("operacje hosta odrzucone: {}/{}", cases.len(), cases.len());
}

#[tokio::test(flavor = "multi_thread")]
async fn owner_denial_reaches_plugin_as_error() {
    let h = harness();
    h.broker
        .script("plugin.napastniczka.probe", ScriptedDecision::NeedsApproval);
    tokio::spawn(approver(h.broker.clone(), ApprovalDecision::Deny));
    let mut c = ctx();
    c.approval_timeout = Duration::from_secs(5);
    let wasm = component(&proxy_body("fs.read-text"));
    h.install(
        probe_manifest(
            "napastniczka",
            &wasm,
            vec![fs_read_tree(NOTES)],
            PluginLimits::default(),
        ),
        wasm,
    )
    .await;
    let out = h
        .tool("plugin_probe")
        .call(json!({"path": NOTE_A}), &c)
        .await;
    assert!(
        out.text.contains("odmowa Brokera") && out.text.contains("odmówił"),
        "{}",
        out.text
    );
    assert_no_leak("odmowa właściciela", &out, &h);
    assert!(h.event_names().contains(&"plugin.host_denied".to_owned()));
}
