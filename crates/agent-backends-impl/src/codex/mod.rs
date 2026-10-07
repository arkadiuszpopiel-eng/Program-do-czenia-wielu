//! Most Codex CLI przez `codex app-server` (JSON-RPC po stdio): `codex exec --json` nie ma
//! kanału zewnętrznych zatwierdzeń, więc tryb „ask” wymaga app-servera (do potwierdzenia
//! w spike (b)). Wątek startuje z `sandbox: workspace-write` i `approvalPolicy: on-request`
//! w izolowanym katalogu; prośby serwera o zatwierdzenie trafiają do `ApprovalSink`.
//! Sterowanie = `turn/interrupt` bieżącej tury + `turn/start` z wiadomością (między krokami).

pub mod parse;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use agent_backends_contract::{AgentEvent, BackendError, BridgeKind, SessionRef, TaskResult};
use mcp_contract::jsonrpc::{Dialect, Message, RequestId, RpcError, parse_line};
use serde_json::{Value, json};
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::lines::{Line, read_line};
use crate::process::spawn_piped;
use crate::runner::{
    Outcome, RunCtx, abort_event, exit_code, kill, over_cost, shutdown, stderr_tail, wall_clock,
};
use parse::{CodexState, TurnEnd, approval_request, approval_response, notification};

/// Argumenty CLI.
pub fn args() -> Vec<String> {
    vec!["app-server".to_owned()]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Initialize,
    Thread,
    Turn,
    Interrupt,
}

struct Session {
    out: mpsc::UnboundedSender<String>,
    next_id: i64,
    purposes: HashMap<RequestId, Purpose>,
    thread: Option<String>,
    turn: Option<String>,
    steer: Vec<String>,
    interrupting: bool,
}

impl Session {
    fn request(&mut self, method: &str, params: Value, purpose: Purpose) {
        self.next_id += 1;
        let id = RequestId::Number(self.next_id);
        self.purposes.insert(id.clone(), purpose);
        let _ = self
            .out
            .send(Message::request(id, method, Some(params)).to_line(Dialect::Lenient));
    }

    fn turn_start(&mut self, text: &str) {
        let thread = self.thread.clone().unwrap_or_default();
        self.request(
            "turn/start",
            json!({"threadId": thread, "input": [{"type": "text", "text": text}]}),
            Purpose::Turn,
        );
    }

    fn interrupt(&mut self) {
        if let (Some(thread), Some(turn)) = (self.thread.clone(), self.turn.clone()) {
            self.interrupting = true;
            self.request(
                "turn/interrupt",
                json!({"threadId": thread, "turnId": turn}),
                Purpose::Interrupt,
            );
        }
    }
}

fn finished(text: String, is_error: bool, subtype: &str, session: Option<SessionRef>) -> Outcome {
    Outcome::Finished(AgentEvent::Done {
        result: TaskResult {
            text,
            is_error,
            subtype: Some(subtype.to_owned()),
            session,
            num_turns: None,
            duration_ms: None,
        },
    })
}

/// Przebieg zadania Codex.
#[allow(clippy::too_many_lines)] // jedna pętla zdarzeń protokołu; podział pogorszyłby czytelność
pub async fn run(
    mut ctx: RunCtx,
    program: PathBuf,
    prompt: String,
    resume: Option<SessionRef>,
    model: Option<String>,
) {
    let spawned_at = Instant::now();
    let mut child = match spawn_piped(&program, &args(), &ctx.workdir) {
        Ok(c) => c,
        Err(error) => {
            ctx.log.emit(AgentEvent::Error { error });
            return;
        }
    };
    let (Some(mut stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        kill(&mut child, ctx.killer.as_ref(), ctx.config.kill_grace).await;
        ctx.log.emit(AgentEvent::Error {
            error: BackendError::Spawn("brak potoków stdio".into()),
        });
        return;
    };
    let stderr = stderr_tail(stderr);
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    let mut s = Session {
        out: out_tx.clone(),
        next_id: 0,
        purposes: HashMap::new(),
        thread: None,
        turn: None,
        steer: Vec::new(),
        interrupting: false,
    };
    s.request(
        "initialize",
        json!({"clientInfo": {"name": "alfa", "title": "Alfa", "version": env!("CARGO_PKG_VERSION")}}),
        Purpose::Initialize,
    );
    let mut state = CodexState::default();
    let mut reader = BufReader::new(stdout);
    let mut session_ref: Option<SessionRef> = None;
    let deadline = wall_clock(&ctx.budget);
    tokio::pin!(deadline);
    let handshake = tokio::time::sleep(ctx.config.codex_handshake_timeout);
    tokio::pin!(handshake);
    let mut started = false;
    let outcome = loop {
        tokio::select! {
            biased;
            () = ctx.cancel.cancelled() => break Outcome::Cancelled,
            () = &mut deadline => break Outcome::Budget("czas ścienny".into()),
            () = &mut handshake, if !started => {
                break Outcome::Finished(AgentEvent::Error {
                    error: BackendError::Protocol("codex app-server nie rozpoczął tury w czasie".into()),
                });
            }
            Some(line) = out_rx.recv() => {
                let mut buf = line.into_bytes();
                buf.push(b'\n');
                if stdin.write_all(&buf).await.is_err() || stdin.flush().await.is_err() {
                    break Outcome::Eof;
                }
            }
            Some(msg) = ctx.steer.recv() => {
                s.steer.push(msg);
                if !s.interrupting {
                    s.interrupt();
                }
            }
            line = read_line(&mut reader, ctx.config.max_line_bytes) => {
                let line = match line {
                    Ok(Some(Line::Text(l))) if l.trim().is_empty() => continue,
                    Ok(Some(Line::Text(l))) => l,
                    Ok(Some(Line::TooLong(n))) => {
                        ctx.log.emit(AgentEvent::Warning { message: format!("pominięto za długą linię CLI ({n} B)") });
                        continue;
                    }
                    Ok(None) | Err(_) => break Outcome::Eof,
                };
                let msg = match parse_line(&line, Dialect::Lenient) {
                    Ok(m) => m,
                    Err(_) => {
                        ctx.log.emit(AgentEvent::Warning {
                            message: format!("linia spoza protokołu app-server ({} B) — pominięta", line.len()),
                        });
                        continue;
                    }
                };
                match msg {
                    Message::Response { id, outcome } => {
                        let purpose = s.purposes.remove(&id);
                        match (purpose, outcome) {
                            (Some(Purpose::Initialize), Ok(_)) => {
                                let ms = u64::try_from(spawned_at.elapsed().as_millis()).unwrap_or(u64::MAX);
                                ctx.log.emit(AgentEvent::ColdStart { ms });
                                let _ = out_tx.send(Message::notification("initialized", None).to_line(Dialect::Lenient));
                                match &resume {
                                    Some(r) => s.request("thread/resume", json!({"threadId": r.id}), Purpose::Thread),
                                    None => {
                                        let mut params = json!({"cwd": ctx.workdir, "sandbox": "workspace-write",
                                                                "approvalPolicy": "on-request"});
                                        if let Some(m) = &model {
                                            params["model"] = json!(m);
                                        }
                                        s.request("thread/start", params, Purpose::Thread);
                                    }
                                }
                            }
                            (Some(Purpose::Thread), Ok(v)) => {
                                let id = v.pointer("/thread/id").and_then(Value::as_str).unwrap_or_default().to_owned();
                                let session = SessionRef { bridge: BridgeKind::Codex, id: id.clone(), workdir: ctx.workdir.clone() };
                                session_ref = Some(session.clone());
                                ctx.log.emit(AgentEvent::SessionStarted { session });
                                s.thread = Some(id);
                                s.turn_start(&prompt);
                            }
                            (Some(Purpose::Turn), Ok(v)) => {
                                started = true;
                                s.turn = v.pointer("/turn/id").and_then(Value::as_str).map(str::to_owned);
                            }
                            (Some(Purpose::Initialize | Purpose::Thread | Purpose::Turn), Err(e)) => {
                                break Outcome::Finished(AgentEvent::Error {
                                    error: BackendError::CliReported(e.message.chars().take(500).collect()),
                                });
                            }
                            _ => {}
                        }
                    }
                    Message::Request { id, method, params } => {
                        let params = params.unwrap_or(Value::Null);
                        match approval_request(&method, &params) {
                            Some(ask) => {
                                let (approvals, log, cancel, out) =
                                    (ctx.approvals.clone(), ctx.log.clone(), ctx.cancel.clone(), out_tx.clone());
                                tokio::spawn(async move {
                                    let decision = approvals.ask(&log, ask, &cancel).await;
                                    let reply = Message::result(id, approval_response(&method, &decision));
                                    let _ = out.send(reply.to_line(Dialect::Lenient));
                                });
                            }
                            None => {
                                ctx.log.emit(AgentEvent::Warning {
                                    message: format!("odrzucono nieobsługiwane żądanie app-server `{}`", method.chars().take(64).collect::<String>()),
                                });
                                let _ = out_tx.send(Message::error(id, RpcError::method_not_found(&method)).to_line(Dialect::Lenient));
                            }
                        }
                    }
                    Message::Notification { method, params } => {
                        let (events, end) = notification(&method, &params.unwrap_or(Value::Null), &mut state);
                        let mut over = false;
                        for ev in events {
                            over |= over_cost(&ctx.budget, &ev);
                            ctx.log.emit(ev);
                        }
                        if over {
                            break Outcome::Budget("koszt".into());
                        }
                        if let Some(TurnEnd { status, error }) = end {
                            s.turn = None;
                            s.interrupting = false;
                            if !s.steer.is_empty() {
                                let msg = s.steer.remove(0);
                                s.turn_start(&msg);
                                continue;
                            }
                            break match status.as_str() {
                                "completed" => finished(std::mem::take(&mut state.last_message), false, "completed", session_ref.clone()),
                                other => finished(error.unwrap_or_default(), true, other, session_ref.clone()),
                            };
                        }
                    }
                }
            }
        }
    };
    if let Some(event) = abort_event(&outcome) {
        kill(&mut child, ctx.killer.as_ref(), ctx.config.kill_grace).await;
        ctx.log.emit(event);
        return;
    }
    if let Outcome::Finished(event) = outcome {
        drop(stdin);
        shutdown(&mut child, ctx.killer.as_ref(), ctx.config.kill_grace).await;
        ctx.log.emit(event);
        return;
    }
    drop(stdin);
    let code = exit_code(&mut child, ctx.killer.as_ref(), ctx.config.kill_grace).await;
    let tail = stderr.await.unwrap_or_default();
    ctx.log.emit(AgentEvent::Error {
        error: BackendError::CliExited {
            code,
            stderr_tail: tail,
        },
    });
}
