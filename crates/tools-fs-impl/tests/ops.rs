//! Operacje `tools-fs`: odczyt (niezaufany, taint), zapis/przeniesienie/kopia/Kosz z krokiem
//! cofania, trwałe usunięcie tylko po zatwierdzeniu, mkdir, zdarzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;

use common::{HOME, ctx, harness};
use safety_broker_contract::{ApprovalDecision, Broker, TaintSource};
use safety_broker_fake::ScriptedDecision;
use serde_json::json;
use tools_common_contract::{DenialReason, ToolErrorKind, ToolStatus, UndoService};
use tools_fs_contract::{FsToolKind as K, INTENT_CONFIRM_DELETE_PERMANENT};
use undo_journal_contract::StepId;

const DOC: &str = "/Users/ala/Documents/notatka.txt";

#[tokio::test]
async fn read_is_untrusted_ranged_and_redacted() {
    let h = harness(&[(
        DOC,
        "linia 1\nklucz=sk-ant-api03-ABCDEFGHIJKLMNOPQRSTUV\nkoniec",
    )]);
    let out = h.call(K::Read, json!({"path": "notatka.txt"})).await;
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(out.untrusted, Some(TaintSource::File));
    assert!(!out.text.contains("sk-ant-api03") && out.text.contains("[ZREDAGOWANO]"));
    assert!(h.broker.session_security(&"s1".into()).tainted);
    let part = h
        .call(K::Read, json!({"path": DOC, "offset": 6, "max_bytes": 3}))
        .await;
    assert_eq!(part.data["content"], "1\nk");
    assert_eq!(part.data["truncated"], true);
    // Limit konfiguracji (64 B) obcina dłuższy odczyt.
    let long = h
        .call(K::Read, json!({"path": DOC, "max_bytes": 100000}))
        .await;
    assert!(long.data["content"].as_str().unwrap().len() <= 64 + 20);
    let missing = h
        .call(K::Read, json!({"path": "/Users/ala/brak.txt"}))
        .await;
    assert_eq!(
        missing.status,
        ToolStatus::Failed {
            error: ToolErrorKind::NotFound
        }
    );
}

#[tokio::test]
async fn binary_and_stat_and_list() {
    let h = harness(&[
        ("/Users/ala/Documents/a.bin", "\u{0}\u{0}\u{1}"),
        ("/Users/ala/Documents/sub/b.txt", "bb"),
        ("/Users/ala/Documents/.ssh/id_rsa", "klucz"),
    ]);
    let bin = h.call(K::Read, json!({"path": "a.bin"})).await;
    assert_eq!(bin.data["binary"], true);
    let st = h.call(K::Stat, json!({"path": "sub/b.txt"})).await;
    assert_eq!(
        (st.data["exists"].clone(), st.data["size"].clone()),
        (json!(true), json!(2))
    );
    assert!(st.untrusted.is_none());
    let dir = h.call(K::Stat, json!({"path": "sub"})).await;
    assert_eq!(dir.data["is_dir"], true);
    let none = h.call(K::Stat, json!({"path": "nie-ma"})).await;
    assert_eq!(none.data["exists"], false);
    let list = h.call(K::List, json!({"path": ".", "depth": 1})).await;
    let paths: Vec<String> = list.data["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap().to_owned())
        .collect();
    // Porównanie po składnikach ścieżki, nie po tekście: `FsPort` składa wpisy `Path::join`,
    // więc na Windows separatorem jest `\` (`…\sub\b.txt`), a na Linuksie `/`.
    assert!(
        paths
            .iter()
            .any(|p| Path::new(p).ends_with(Path::new("sub").join("b.txt"))),
        "{paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p.contains(".ssh")),
        "deny-lista ukryta: {paths:?}"
    );
    let limited = h
        .call(K::List, json!({"path": ".", "depth": 1, "limit": 1}))
        .await;
    assert_eq!(limited.data["truncated"], true);
    let bad = h.call(K::List, json!({"path": "/Users/ala/brak"})).await;
    assert_eq!(
        bad.status,
        ToolStatus::Failed {
            error: ToolErrorKind::NotFound
        }
    );
}

#[tokio::test]
async fn search_by_name_and_content() {
    let h = harness(&[
        ("/Users/ala/Documents/f1.pdf", "x"),
        ("/Users/ala/Documents/deep/f2.PDF", "y"),
        ("/Users/ala/Documents/n.txt", "Termin: piątek\nhasło: tajne"),
        ("/Users/ala/Documents/.claude/x.pdf", "z"),
    ]);
    let out = h
        .call(K::Search, json!({"root": ".", "pattern": "*.pdf"}))
        .await;
    assert_eq!(
        out.data["hits"].as_array().unwrap().len(),
        2,
        "{}",
        out.text
    );
    let c = h
        .call(
            K::Search,
            json!({"root": ".", "pattern": "*.txt", "content": "TERMIN"}),
        )
        .await;
    assert_eq!(c.data["hits"][0]["line"], "Termin: piątek");
    let s = h
        .call(
            K::Search,
            json!({"root": ".", "pattern": "*", "content": "hasło"}),
        )
        .await;
    assert!(s.text.contains("[ZREDAGOWANO]") && !s.text.contains("tajne"));
    let shallow = h
        .call(
            K::Search,
            json!({"root": ".", "pattern": "*.pdf", "max_depth": 0}),
        )
        .await;
    assert_eq!(shallow.data["hits"].as_array().unwrap().len(), 1);
    let empty = h
        .call(K::Search, json!({"root": ".", "pattern": " "}))
        .await;
    assert_eq!(
        empty.status,
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs
        }
    );
}

#[tokio::test]
async fn write_modes_and_undo() {
    let h = harness(&[(DOC, "stare")]);
    let exists = h
        .call(K::Write, json!({"path": DOC, "content": "nowe"}))
        .await;
    assert_eq!(
        exists.status,
        ToolStatus::Failed {
            error: ToolErrorKind::AlreadyExists
        }
    );
    let over = h
        .call(
            K::Write,
            json!({"path": DOC, "content": "nowe", "mode": "overwrite"}),
        )
        .await;
    assert!(over.is_ok(), "{over:?}");
    let undo = over.undo.clone().unwrap();
    assert_eq!(undo.service, UndoService::Journal);
    assert_eq!(h.file(DOC).as_deref(), Some("nowe"));
    let app = h
        .call(
            K::Write,
            json!({"path": DOC, "content": "+", "mode": "append"}),
        )
        .await;
    assert_eq!(h.file(DOC).as_deref(), Some("nowe+"));
    h.journal.undo(StepId(app.undo.unwrap().id)).unwrap();
    h.journal.undo(StepId(undo.id)).unwrap();
    assert_eq!(h.file(DOC).as_deref(), Some("stare"));
    let new = h
        .call(K::Write, json!({"path": "nowy/plik.txt", "content": "a"}))
        .await;
    assert!(new.is_ok());
    assert_eq!(
        h.file("/Users/ala/Documents/nowy/plik.txt").as_deref(),
        Some("a")
    );
    assert!(new.untrusted.is_none());
}

#[tokio::test]
async fn move_copy_rename_delete_with_undo() {
    let h = harness(&[(DOC, "x")]);
    let before = h.fs.snapshot();
    let mv = h
        .call(
            K::Move,
            json!({"from": DOC, "to": "/Users/ala/Desktop/n.txt"}),
        )
        .await;
    assert!(mv.is_ok(), "{mv:?}");
    let cp = h
        .call(
            K::Copy,
            json!({"from": "/Users/ala/Desktop/n.txt", "to": "kopia.txt"}),
        )
        .await;
    assert!(cp.is_ok(), "{cp:?}");
    let rn = h
        .call(
            K::Rename,
            json!({"path": "kopia.txt", "new_name": "k2.txt"}),
        )
        .await;
    assert!(rn.is_ok(), "{rn:?}");
    assert!(h.file("/Users/ala/Documents/k2.txt").is_some());
    let del = h.call(K::Delete, json!({"path": "k2.txt"})).await;
    assert!(del.is_ok(), "{del:?}");
    assert_eq!(h.fs.recycle_bin().len(), 1);
    let bad_name = h
        .call(K::Rename, json!({"path": "x", "new_name": "../y"}))
        .await;
    assert_eq!(
        bad_name.status,
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs
        }
    );
    let reports = h.journal.undo_last(&"s1".into(), 4).unwrap();
    assert_eq!(reports.len(), 4);
    assert_eq!(h.fs.snapshot(), before);
}

#[tokio::test]
async fn delete_permanent_always_needs_owner() {
    let h = harness(&[(DOC, "x")]);
    let out = h.call(K::DeletePermanent, json!({"path": DOC})).await;
    assert_eq!(out.status, ToolStatus::NeedsConfirmation);
    assert_eq!(
        out.intent.as_ref().unwrap().kind,
        INTENT_CONFIRM_DELETE_PERMANENT
    );
    assert!(h.file(DOC).is_some(), "bez potwierdzenia nic nie usunięto");
    // Broker pyta → właściciel odmawia.
    h.broker
        .script("tools-fs.delete_permanent", ScriptedDecision::NeedsApproval);
    let b = h.broker.clone();
    let deny = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        b.auto_approve(ApprovalDecision::Deny).await
    });
    let mut c = ctx();
    c.approval_timeout = std::time::Duration::from_secs(5);
    let denied = h
        .call_ctx(K::DeletePermanent, json!({"path": DOC}), &c)
        .await;
    deny.await.unwrap();
    assert!(matches!(
        denied.status,
        ToolStatus::Denied {
            reason: DenialReason::OwnerDenied { .. }
        }
    ));
    assert!(denied.text.contains("Odmowa"));
    // Zatwierdzenie → usunięte, z krokiem w dzienniku (pre-image).
    let b = h.broker.clone();
    let ok = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        b.auto_approve(ApprovalDecision::Allow).await
    });
    let done = h
        .call_ctx(K::DeletePermanent, json!({"path": DOC}), &c)
        .await;
    ok.await.unwrap();
    assert!(done.is_ok(), "{done:?}");
    assert!(done.approval.is_some());
    assert!(h.file(DOC).is_none());
    h.journal.undo(StepId(done.undo.unwrap().id)).unwrap();
    assert_eq!(h.file(DOC).as_deref(), Some("x"));
}

#[tokio::test]
async fn mkdir_is_idempotent_and_honest() {
    let h = harness(&[(DOC, "x")]);
    let existing = h.call(K::Mkdir, json!({"path": "."})).await;
    assert!(existing.is_ok() && existing.undo.is_none());
    let file = h.call(K::Mkdir, json!({"path": DOC})).await;
    assert_eq!(
        file.status,
        ToolStatus::Failed {
            error: ToolErrorKind::AlreadyExists
        }
    );
    let new = h.call(K::Mkdir, json!({"path": "Faktury"})).await;
    assert!(new.is_ok() && new.undo.is_some(), "{new:?}");
    assert!(
        h.fs.snapshot()
            .keys()
            .all(|p| !p.to_string_lossy().contains(tools_fs_impl::MKDIR_MARKER))
    );
}

#[tokio::test]
async fn events_are_published() {
    let h = harness(&[(DOC, "x")]);
    h.call(K::Read, json!({"path": DOC})).await;
    h.call(K::Read, json!({"path": format!("{HOME}/.ssh/id_rsa")}))
        .await;
    let kinds: Vec<String> = h
        .bus
        .recorded()
        .iter()
        .map(|e| e.kind.as_str().to_owned())
        .collect();
    assert!(kinds.contains(&"tool.fs.result".to_owned()), "{kinds:?}");
    assert!(
        kinds.contains(&"tool.fs.denylist_hit".to_owned()),
        "{kinds:?}"
    );
}

#[tokio::test]
async fn cancelled_context_does_nothing() {
    let h = harness(&[(DOC, "x")]);
    let c = ctx();
    c.cancel.cancel();
    let out = h.call_ctx(K::Delete, json!({"path": DOC}), &c).await;
    assert_eq!(out.status, ToolStatus::Cancelled);
    assert!(h.file(DOC).is_some());
    assert!(h.journal.steps(&"s1".into()).is_empty());
}
