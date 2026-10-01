//! Most Claude Code: `claude -p` w trybie stream-json (wejście i wyjście), prośby o uprawnienia
//! przez `--permission-prompt-tool mcp__alfa__approve` (serwer MCP Alfy → `ApprovalSink`),
//! sterowanie kolejnymi wiadomościami użytkownika na stdin, wznawianie `--resume`.

pub mod parse;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use agent_backends_contract::{
    AgentEvent, ApprovalDecision, BackendError, BridgeKind, PermissionKind, TaskSpec,
};
use async_trait::async_trait;
use mcp_contract::{
    ApprovalRouter, PERMISSION_PROMPT_TOOL, PermissionPromptRequest, PermissionPromptResponse,
};
use providers_contract::CancellationToken;
use serde_json::json;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;

use crate::approvals::{Approvals, Ask};
use crate::lines::{Line, read_line};
use crate::log::TaskLog;
use crate::process::spawn_piped;
use crate::runner::{
    Outcome, RunCtx, abort_event, exit_code, kill, over_cost, stderr_tail, wall_clock,
};
use parse::ClaudeParser;

/// Argumenty CLI (bez programu). `mcp_config` — plik konfiguracji MCP z serwerem Alfy.
pub fn args(spec: &TaskSpec, mcp_config: &Path, partial_messages: bool) -> Vec<String> {
    let mut a: Vec<String> = [
        "-p",
        "--output-format",
        "stream-json",
        "--input-format",
        "stream-json",
        "--verbose",
        "--permission-mode",
        "default",
        "--permission-prompt-tool",
        PERMISSION_PROMPT_TOOL,
        "--mcp-config",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    a.push(mcp_config.to_string_lossy().into_owned());
    a.push("--strict-mcp-config".into());
    if partial_messages {
        a.push("--include-partial-messages".into());
    }
    if !spec.allowed_tools.is_empty() {
        a.extend(["--allowedTools".into(), spec.allowed_tools.join(",")]);
    }
    if !spec.disallowed_tools.is_empty() {
        a.extend(["--disallowedTools".into(), spec.disallowed_tools.join(",")]);
    }
    if let Some(n) = spec.budget.max_turns {
        a.extend(["--max-turns".into(), n.to_string()]);
    }
    if let Some(model) = &spec.model {
        a.extend(["--model".into(), model.clone()]);
    }
    if let Some(session) = &spec.session {
        a.extend(["--resume".into(), session.id.clone()]);
    }
    a
}

/// Wiadomość użytkownika w formacie `--input-format stream-json`.
pub fn user_message(text: &str) -> String {
    json!({"type": "user", "message": {"role": "user", "content": [{"type": "text", "text": text}]}})
        .to_string()
}

/// Router zatwierdzeń zadania: `approve` z MCP → hub zatwierdzeń.
pub struct ClaudeApprovals {
    /// Hub.
    pub approvals: Arc<Approvals>,
    /// Dziennik zadania.
    pub log: Arc<TaskLog>,
    /// Anulowanie zadania.
    pub cancel: CancellationToken,
}

#[async_trait]
impl ApprovalRouter for ClaudeApprovals {
    async fn permission_prompt(
        &self,
        request: PermissionPromptRequest,
    ) -> PermissionPromptResponse {
        if self.log.is_finished() {
            return PermissionPromptResponse::Deny {
                message: "zadanie zakończone".into(),
            };
        }
        let ask = Ask {
            bridge: BridgeKind::ClaudeCode,
            kind: PermissionKind::Tool,
            tool: request.tool_name.clone(),
            input: request.input.clone(),
            reason: None,
            call_id: request.tool_use_id.clone(),
        };
        match self.approvals.ask(&self.log, ask, &self.cancel).await {
            ApprovalDecision::Allow { updated_input } => PermissionPromptResponse::Allow {
                updated_input: updated_input.unwrap_or(request.input),
            },
            ApprovalDecision::Deny { message } => PermissionPromptResponse::Deny { message },
        }
    }
}

async fn send(stdin: &mut Option<ChildStdin>, line: &str) -> bool {
    let Some(w) = stdin.as_mut() else {
        return false;
    };
    let mut buf = line.as_bytes().to_vec();
    buf.push(b'\n');
    w.write_all(&buf).await.is_ok() && w.flush().await.is_ok()
}

/// Przebieg zadania Claude Code.
pub async fn run(mut ctx: RunCtx, program: PathBuf, args: Vec<String>, prompt: String) {
    let spawned_at = Instant::now();
    let mut child = match spawn_piped(&program, &args, &ctx.workdir) {
        Ok(c) => c,
        Err(error) => {
            ctx.log.emit(AgentEvent::Error { error });
            return;
        }
    };
    let (Some(stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        kill(&mut child, ctx.killer.as_ref(), ctx.config.kill_grace).await;
        ctx.log.emit(AgentEvent::Error {
            error: BackendError::Spawn("brak potoków stdio".into()),
        });
        return;
    };
    let stderr = stderr_tail(stderr);
    let mut stdin = Some(stdin);
    let mut sent = u32::from(send(&mut stdin, &user_message(&prompt)).await);
    let mut results = 0u32;
    let mut reader = BufReader::new(stdout);
    let mut parser = ClaudeParser::new(&ctx.workdir);
    let mut cold = false;
    let deadline = wall_clock(&ctx.budget);
    tokio::pin!(deadline);
    let outcome = loop {
        tokio::select! {
            biased;
            () = ctx.cancel.cancelled() => break Outcome::Cancelled,
            () = &mut deadline => break Outcome::Budget("czas ścienny".into()),
            Some(msg) = ctx.steer.recv() => {
                if send(&mut stdin, &user_message(&msg)).await {
                    sent += 1;
                } else {
                    ctx.log.emit(AgentEvent::Warning {
                        message: "sterowanie odrzucone: wejście CLI jest już zamknięte".into(),
                    });
                }
            }
            line = read_line(&mut reader, ctx.config.max_line_bytes) => match line {
                Ok(Some(Line::Text(l))) if l.trim().is_empty() => {}
                Ok(Some(Line::Text(l))) => {
                    let parsed = parser.line(&l);
                    if parsed.json && !cold {
                        cold = true;
                        let ms = u64::try_from(spawned_at.elapsed().as_millis()).unwrap_or(u64::MAX);
                        ctx.log.emit(AgentEvent::ColdStart { ms });
                    }
                    let mut over = false;
                    for ev in parsed.events {
                        over |= over_cost(&ctx.budget, &ev);
                        ctx.log.emit(ev);
                    }
                    if over {
                        break Outcome::Budget("koszt".into());
                    }
                    if parsed.result {
                        results += 1;
                        while let Ok(msg) = ctx.steer.try_recv() {
                            if send(&mut stdin, &user_message(&msg)).await {
                                sent += 1;
                            }
                        }
                        if results >= sent {
                            // Wszystkie wiadomości obsłużone — EOF na stdin kończy CLI.
                            stdin = None;
                        }
                    }
                }
                Ok(Some(Line::TooLong(n))) => {
                    ctx.log.emit(AgentEvent::Warning {
                        message: format!("pominięto za długą linię CLI ({n} B)"),
                    });
                }
                Ok(None) | Err(_) => break Outcome::Eof,
            },
        }
    };
    drop(stdin);
    if let Some(event) = abort_event(&outcome) {
        kill(&mut child, ctx.killer.as_ref(), ctx.config.kill_grace).await;
        ctx.log.emit(event);
        return;
    }
    let code = exit_code(&mut child, ctx.killer.as_ref(), ctx.config.kill_grace).await;
    let tail = stderr.await.unwrap_or_default();
    let event = match parser.last_result() {
        Some(result) => AgentEvent::Done {
            result: result.clone(),
        },
        None => AgentEvent::Error {
            error: BackendError::CliExited {
                code,
                stderr_tail: tail,
            },
        },
    };
    ctx.log.emit(event);
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_backends_contract::SessionRef;
    use core_bus_contract::SessionId;

    #[test]
    fn args_contain_permission_prompt_tool_and_options() {
        let mut spec =
            TaskSpec::user_request(BridgeKind::ClaudeCode, "p", "/src", SessionId::new("s"));
        spec.allowed_tools = vec!["Read".into(), "Grep".into()];
        spec.disallowed_tools = vec!["WebFetch".into()];
        spec.budget.max_turns = Some(5);
        spec.model = Some("sonnet".into());
        spec.session = Some(SessionRef {
            bridge: BridgeKind::ClaudeCode,
            id: "abc".into(),
            workdir: "/wt".into(),
        });
        let a = args(&spec, Path::new("/rt/mcp.json"), true);
        let joined = a.join(" ");
        assert!(
            joined
                .starts_with("-p --output-format stream-json --input-format stream-json --verbose")
        );
        assert!(joined.contains("--permission-mode default"));
        assert!(joined.contains("--permission-prompt-tool mcp__alfa__approve"));
        assert!(joined.contains("--mcp-config /rt/mcp.json --strict-mcp-config"));
        assert!(joined.contains("--allowedTools Read,Grep"));
        assert!(joined.contains("--disallowedTools WebFetch"));
        assert!(joined.contains("--max-turns 5"));
        assert!(joined.contains("--resume abc"));
        assert!(
            !a.iter()
                .any(|x| x.contains("dangerously") || *x == ["bypass", "Permissions"].concat())
        );
        let minimal = args(
            &TaskSpec::user_request(BridgeKind::ClaudeCode, "p", "/s", SessionId::new("s")),
            Path::new("m"),
            false,
        );
        assert!(
            !minimal
                .iter()
                .any(|x| x.starts_with("--allowed") || x == "--include-partial-messages")
        );
    }

    #[test]
    fn user_message_is_single_line_json() {
        let m = user_message("a\nb");
        assert!(!m.contains('\n'));
        let v: serde_json::Value = serde_json::from_str(&m).unwrap();
        assert_eq!(v["message"]["content"][0]["text"], "a\nb");
    }
}
