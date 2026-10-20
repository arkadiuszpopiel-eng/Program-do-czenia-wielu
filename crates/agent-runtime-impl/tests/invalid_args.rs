//! Niepoprawne argumenty wywołań (C2.3): wywołanie z `ToolArguments::Invalid` (ucięty JSON,
//! argumenty niezgodne ze schematem) nie trafia do narzędzia — model dostaje wynik z błędem
//! i prośbą o ponowienie, krok kończy się błędem w osi czasu; poprawne wywołania tej samej tury
//! działają normalnie; przebieg kończy się bez paniki.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use agent_runtime_contract::{AgentRuntime, RunEvent, RunOutcome, StepStatus};
use common::{answer, call, spec, world};
use providers_contract::{
    ContentBlock, ProviderEvent, Role, StopReason, ToolArguments, ToolResult, ToolResultPart, Usage,
};
use providers_fake::{FAKE_MODEL, Script, Step};
use serde_json::json;
use tools_common_contract::ToolOutcome;
use tools_fs_contract::FsToolKind;

/// Tura z wywołaniami: `(id, narzędzie, argumenty)`.
fn turn(calls: &[(&str, &str, ToolArguments)]) -> Script {
    let mut steps = vec![Step::Emit(ProviderEvent::Started {
        model: FAKE_MODEL.into(),
        response_id: None,
    })];
    for (index, (id, name, arguments)) in (0u32..).zip(calls) {
        steps.push(Step::Emit(ProviderEvent::ToolCallStart {
            index,
            id: (*id).into(),
            name: (*name).into(),
        }));
        steps.push(Step::Emit(ProviderEvent::ToolCallEnd {
            index,
            id: (*id).into(),
            arguments: arguments.clone(),
        }));
    }
    steps.push(Step::Emit(ProviderEvent::Usage(Usage {
        input_tokens: 10,
        output_tokens: 5,
        ..Usage::default()
    })));
    steps.push(Step::Emit(ProviderEvent::stop(StopReason::ToolUse)));
    Script::new(steps)
}

fn results(message: &providers_contract::Message) -> Vec<ToolResult> {
    message
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolResult(r) => Some(r.clone()),
            _ => None,
        })
        .collect()
}

fn result_text(r: &ToolResult) -> String {
    r.content
        .iter()
        .filter_map(|p| match p {
            ToolResultPart::Text { text } => Some(text.as_str()),
            ToolResultPart::Image { .. } => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn broken_arguments_go_back_to_the_model_and_the_retry_runs() {
    let w = world();
    let broken = ToolArguments::from_raw("{\"path\": \"/Users/ala/Documents/a.txt\", \"conte");
    assert!(matches!(broken, ToolArguments::Invalid { .. }));
    w.provider.push_script(turn(&[("t1", "fs_write", broken)]));
    w.provider.push_script(call(
        "t2",
        "fs_write",
        json!({"path": "/Users/ala/Documents/a.txt", "content": "x"}),
    ));
    w.provider.push_script(answer("Zapisałam."));
    let run = w.runtime.start(spec()).await.unwrap();
    let outcome = w.runtime.wait(&run).await.unwrap();
    assert!(
        matches!(outcome, RunOutcome::Completed { .. }),
        "{outcome:?}"
    );
    assert_eq!(
        w.fs.calls(FsToolKind::Write).len(),
        1,
        "narzędzie uruchomione tylko z poprawionymi argumentami"
    );
    let reqs = w.provider.requests();
    let retry = reqs[1].messages.last().unwrap();
    assert_eq!(retry.role, Role::User);
    let got = results(retry);
    assert_eq!(got.len(), 1);
    assert!(got[0].is_error && got[0].tool_use_id == "t1");
    let text = result_text(&got[0]);
    assert!(
        text.contains("argumenty wywołania `fs_write` są niepoprawne")
            && text.contains("EOF while parsing")
            && text.contains("wywołaj je ponownie"),
        "{text}"
    );
    let events = w.runtime.events(&run).unwrap();
    assert!(events.iter().any(|e| matches!(
        &e.event,
        RunEvent::StepFinished { status: StepStatus::Failed, tool: Some(t), output, .. }
            if t == "fs_write" && output.contains("niepoprawne")
    )));
}

#[tokio::test(start_paused = true)]
async fn valid_calls_of_the_same_turn_still_run() {
    let w = world();
    w.fs.push(FsToolKind::List, ToolOutcome::ok("faktura.pdf", json!({})));
    let schema_error = ToolArguments::Invalid {
        raw: "{\"path\": 7}".into(),
        error: "argumenty niezgodne ze schematem narzędzia: /path: 7 is not of type \"string\""
            .into(),
    };
    w.provider.push_script(turn(&[
        (
            "a",
            "fs_list",
            ToolArguments::Parsed {
                value: json!({"path": "/Users/ala/Documents"}),
            },
        ),
        ("b", "fs_write", schema_error),
    ]));
    w.provider.push_script(answer("Gotowe."));
    let run = w.runtime.start(spec()).await.unwrap();
    assert!(matches!(
        w.runtime.wait(&run).await.unwrap(),
        RunOutcome::Completed { .. }
    ));
    assert_eq!(w.fs.calls(FsToolKind::List).len(), 1);
    assert!(w.fs.calls(FsToolKind::Write).is_empty());
    let reqs = w.provider.requests();
    let got = results(reqs[1].messages.last().unwrap());
    assert_eq!(got.len(), 2, "wyniki obu wywołań w jednej wiadomości");
    assert!(!got[0].is_error && got[0].tool_use_id == "a");
    assert!(got[1].is_error && got[1].tool_use_id == "b");
    assert!(result_text(&got[1]).contains("is not of type"));
}

/// Regresja 0.0.64 (próba obowiązków): odrzucone wywołania nie liczą się do wykrywania pętli —
/// model, który kilka razy z rzędu poda te same złe argumenty, nadal może się poprawić.
#[tokio::test(start_paused = true)]
async fn rejected_calls_do_not_count_as_loop_repeats() {
    let w = world();
    for id in ["z1", "z2", "z3", "z4"] {
        let broken = ToolArguments::from_raw("{\"path\": \"/Users/ala/Documents/a.txt\", \"conte");
        w.provider.push_script(turn(&[(id, "fs_write", broken)]));
    }
    w.provider.push_script(call(
        "ok",
        "fs_write",
        json!({"path": "/Users/ala/Documents/a.txt", "content": "x"}),
    ));
    w.provider.push_script(answer("Zapisałam."));
    let run = w.runtime.start(spec()).await.unwrap();
    let outcome = w.runtime.wait(&run).await.unwrap();
    assert!(
        matches!(outcome, RunOutcome::Completed { .. }),
        "{outcome:?}"
    );
    assert_eq!(w.fs.calls(FsToolKind::Write).len(), 1);
    let events = w.runtime.events(&run).unwrap();
    assert!(
        !events
            .iter()
            .any(|e| matches!(&e.event, RunEvent::LoopDetected { .. })),
        "odrzucenia nie są powtórzeniami"
    );
}
