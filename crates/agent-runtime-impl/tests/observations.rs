//! Końcówka błędu kompilatora musi dojść do modelu także po dwóch ograniczeniach wyjścia.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use agent_runtime_contract::AgentRuntime;
use common::{answer, call, spec, world};
use providers_contract::{ContentBlock, ToolResultPart};
use safety_broker_contract::TaintSource;
use serde_json::json;
use tools_common_contract::ToolOutcome;
use tools_fs_contract::FsToolKind;

#[tokio::test(start_paused = true)]
async fn compiler_error_at_the_end_of_a_large_log_reaches_the_model() {
    let w = world();
    let header = "Testy nie wystartowały: kompilacja zakończona kodem101.\n";
    let error = "error[E0308]: expected `u32`, found `&str` at src/main.rs:2";
    let output = format!(
        "{header}{}{error}",
        "kompilowanie zależności…\n".repeat(1250)
    );
    assert!(output.chars().count() > 30_000);
    w.fs.push(
        FsToolKind::Read,
        ToolOutcome::ok(output, json!({"exit_code":101})).untrusted(TaintSource::File),
    );
    w.provider.push_script(call(
        "build-log",
        "fs_read",
        json!({"path":"/Users/ala/Documents/build.log"}),
    ));
    w.provider
        .push_script(answer("Kompilacja ma błąd typu; wymaga poprawy."));
    let run = w.runtime.start(spec()).await.unwrap();
    w.runtime.wait(&run).await.unwrap();
    let requests = w.provider.requests();
    let observed = requests[1]
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|c| match c {
            ContentBlock::ToolResult(r) => Some(
                r.content
                    .iter()
                    .filter_map(|p| match p {
                        ToolResultPart::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect::<String>();
    assert!(
        observed.contains(header.trim()),
        "nagłówek wyniku musi zostać"
    );
    assert!(
        observed.contains(error),
        "końcowy błąd kompilatora zaginął: {observed}"
    );
}
