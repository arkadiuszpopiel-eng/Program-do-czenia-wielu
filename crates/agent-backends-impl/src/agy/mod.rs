//! Most Antigravity CLI (`agy`, następca Gemini CLI dla kont Google AI Pro): polecenie jednorazowe
//! `agy -p <polecenie> --output-format stream-json --sandbox`, wznawianie `--conversation <id>`.
//!
//! Trasa szara, domyślnie wyłączona (regulamin Antigravity, pkt 6) — włącza ją właściciel
//! świadomie w Ustawieniach → Mosty CLI. Bez flag automatycznej zgody: w trybie `-p` agy sam
//! odrzuca polecenia powłoki wymagające zgody, a `--sandbox` dokłada ograniczenia terminala.
//! Sterowania w trakcie nie ma (polecenie jednorazowe) — dosłane wiadomości kończą się
//! ostrzeżeniem.

pub mod parse;

use std::path::PathBuf;
use std::time::Instant;

use agent_backends_contract::{AgentEvent, BackendError, TaskSpec};

use crate::lines::{Line, read_line};
use crate::process::spawn_piped;
use crate::runner::{
    Outcome, RunCtx, abort_event, exit_code, kill, over_cost, stderr_tail, wall_clock,
};
use parse::AgyParser;

/// Najdłuższe polecenie przekazywane w argumencie (limit wiersza poleceń Windows to 32 767 znaków).
pub const MAX_PROMPT_CHARS: usize = 24_000;

/// Argumenty CLI (bez programu).
pub fn args(spec: &TaskSpec, prompt: &str) -> Result<Vec<String>, BackendError> {
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        return Err(BackendError::InvalidSpec(format!(
            "polecenie dla Antigravity jest za długie (najwyżej {MAX_PROMPT_CHARS} znaków)"
        )));
    }
    let mut a: Vec<String> = vec![
        "-p".into(),
        prompt.to_owned(),
        "--output-format".into(),
        "stream-json".into(),
        "--sandbox".into(),
        "--disable-slash-commands".into(),
    ];
    if let Some(model) = &spec.model {
        a.extend(["--model".into(), model.clone()]);
    }
    if let Some(session) = &spec.session {
        // Identyfikator pochodzi z wyjścia CLI — tylko bezpieczne znaki, bez `-` na początku.
        let id = &session.id;
        if id.is_empty()
            || id.starts_with('-')
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(BackendError::InvalidSpec(
                "niepoprawny identyfikator rozmowy Antigravity".into(),
            ));
        }
        a.extend(["--conversation".into(), id.clone()]);
    }
    Ok(a)
}

/// Przebieg zadania Antigravity.
pub async fn run(mut ctx: RunCtx, program: PathBuf, args: Vec<String>) {
    let spawned_at = Instant::now();
    let mut child = match spawn_piped(&program, &args, &ctx.workdir) {
        Ok(c) => c,
        Err(error) => {
            ctx.log.emit(AgentEvent::Error { error });
            return;
        }
    };
    // Polecenie jest w argumencie — puste wejście, żeby CLI na nie nie czekało.
    drop(child.stdin.take());
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        kill(&mut child, ctx.killer.as_ref(), ctx.config.kill_grace).await;
        ctx.log.emit(AgentEvent::Error {
            error: BackendError::Spawn("brak potoków stdio".into()),
        });
        return;
    };
    let stderr = stderr_tail(stderr);
    let mut reader = tokio::io::BufReader::new(stdout);
    let mut parser = AgyParser::new(&ctx.workdir);
    let mut cold = false;
    let deadline = wall_clock(&ctx.budget);
    tokio::pin!(deadline);
    let outcome = loop {
        tokio::select! {
            biased;
            () = ctx.cancel.cancelled() => break Outcome::Cancelled,
            () = &mut deadline => break Outcome::Budget("czas ścienny".into()),
            Some(_) = ctx.steer.recv() => {
                ctx.log.emit(AgentEvent::Warning {
                    message: "Antigravity nie przyjmuje wiadomości w trakcie zadania — wyślij je jako nowe zadanie".into(),
                });
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
    use agent_backends_contract::{BridgeKind, SessionRef};
    use core_bus_contract::SessionId;

    #[test]
    fn args_are_one_shot_sandboxed_and_never_auto_approve() {
        let mut spec =
            TaskSpec::user_request(BridgeKind::Antigravity, "p", "/src", SessionId::new("s"));
        spec.model = Some("gemini-3-pro".into());
        spec.session = Some(SessionRef {
            bridge: BridgeKind::Antigravity,
            id: "conv-7".into(),
            workdir: "/wt".into(),
        });
        let a = args(&spec, "napisz test").unwrap();
        assert_eq!(&a[..2], ["-p", "napisz test"]);
        let joined = a.join(" ");
        assert!(joined.contains("--output-format stream-json"));
        assert!(joined.contains("--sandbox"));
        assert!(joined.contains("--model gemini-3-pro"));
        assert!(joined.contains("--conversation conv-7"));
        assert!(!a.iter().any(|x| x.contains("dangerously") || x == "--mode"));
        assert!(args(&spec, &"x".repeat(MAX_PROMPT_CHARS + 1)).is_err());
        spec.session = Some(SessionRef {
            bridge: BridgeKind::Antigravity,
            id: "--mode".into(),
            workdir: "/wt".into(),
        });
        assert!(args(&spec, "p").is_err());
    }
}
