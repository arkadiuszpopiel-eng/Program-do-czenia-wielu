//! `shell_run`: analiza polecenia → deny-lista i polityka (bez Brokera) → zgody Brokera
//! (`shell.exec(cwd)` z poleceniem do reguł Jądra, zakresy ścieżek spoza `cwd`, `net.egress`
//! dla hostów) → snapshot zakresu (brak snapshotu = brak wykonania) → `ExecPort` w Job Object
//! (rejestr kill-switcha, anulowanie) → zatwierdzenie kroku → wyjście zredagowane i niezaufane.

use std::path::PathBuf;
use std::sync::Arc;

use core_bus_contract::Level;
use platform_contract::{
    ExecControl, ExecOutput, ExecSpec, ExecTermination, Integrity, PlatformError, ProcessSpec,
    filter_env,
};
use risk_classifier_contract::Reversibility;
use safety_broker_contract::{Capability, HostPattern, TaintSource};
use tools_common_contract::{
    Authorization, DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, ToolStatus,
    UndoRef, UndoService, base_facts, parse_args, paths, report_untrusted, text,
};
use tools_shell_contract::{
    EVENT_DENIED, EVENT_EXITED, EVENT_KILLED, EVENT_STARTED, MAX_COMMAND_CHARS, RunArgs, RunOutput,
    Termination, analyze,
};
use undo_journal_contract::{StepCtx, StepId, UndoError};
use watchdog_contract::ProcessRole;

use crate::Core;

type Step<T> = Result<T, Box<ToolOutcome>>;

fn fail(kind: ToolErrorKind, text: String) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::failed(kind, text))
}

impl Core {
    async fn deny(
        &self,
        ctx: &ToolCtx,
        reason: DenialReason,
        action: &str,
        why: &str,
    ) -> Box<ToolOutcome> {
        let payload = serde_json::json!({ "reason": reason, "detail": why });
        self.emit(EVENT_DENIED, Level::Warn, payload, ctx).await;
        let mut out = ToolOutcome::denied(reason, action);
        if !why.is_empty() {
            out.text = format!("{} ({why})", out.text);
        }
        Box::new(out)
    }

    /// Zgody na wszystkie zdolności polecenia.
    async fn capabilities(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        cwd: &str,
        command: &str,
    ) -> Step<Vec<Authorization>> {
        let action = format!("polecenie „{}”", text::truncate_chars(command, 80).0);
        let a = analyze(command);
        if a.credential_path {
            return Err(self
                .deny(ctx, DenialReason::DenyList, &action, "ścieżka poświadczeń")
                .await);
        }
        if a.interactive {
            return Err(self
                .deny(
                    ctx,
                    DenialReason::Policy,
                    &action,
                    "polecenie wymaga interakcji — użyj `shell_terminal`",
                )
                .await);
        }
        if a.network && a.hosts.is_empty() {
            return Err(self
                .deny(
                    ctx,
                    DenialReason::Policy,
                    &action,
                    "polecenie sieciowe bez jawnego hosta — podaj adres wprost",
                )
                .await);
        }
        let cwd_scope = paths::tree_scope(cwd, &self.env).map_err(|e| {
            fail(
                ToolErrorKind::InvalidArgs,
                format!("Niepoprawny katalog roboczy: {e}."),
            )
        })?;
        let mut outside = Vec::new();
        for raw in &a.paths {
            let Ok(resolved) = paths::resolve_path(raw, Some(cwd), &self.env) else {
                continue;
            };
            if self.denied(&resolved) {
                return Err(self
                    .deny(
                        ctx,
                        DenialReason::DenyList,
                        &action,
                        "ścieżka na deny-liście",
                    )
                    .await);
            }
            if let Ok(scope) = paths::tree_scope(&resolved, &self.env)
                && !scope.is_subset_of(&cwd_scope)
            {
                outside.push(scope);
            }
        }
        let mut facts = base_facts(m, ctx);
        facts.destructive = a.destructiveness(outside.is_empty());
        facts.install = a.install;
        facts.command = Some(command.to_owned());
        let mut caps = vec![(Capability::ShellExec(cwd_scope), facts.clone())];
        for scope in outside {
            let mut f = facts.clone();
            f.reversible = Reversibility::No;
            caps.push((Capability::ShellExec(scope), f));
        }
        for host in &a.hosts {
            let pattern = HostPattern::parse(host).map_err(|e| {
                fail(
                    ToolErrorKind::InvalidArgs,
                    format!("Niepoprawny host: {e}."),
                )
            })?;
            let mut f = facts.clone();
            f.reversible = Reversibility::No;
            caps.push((Capability::NetEgress(pattern), f));
        }
        self.authorize(ctx, caps, &action).await
    }

    fn snapshot(&self, ctx: &ToolCtx, m: &ToolManifest, cwd: &str) -> Step<StepId> {
        let step_ctx = StepCtx {
            session: ctx.holder.session.clone(),
            agent: ctx.holder.agent.clone(),
            run: ctx.run.clone(),
            turn: None,
            label: ctx.undo_label(m),
            allow_irreversible: false,
        };
        let step = self.journal.begin_step(step_ctx).map_err(|e| {
            fail(
                ToolErrorKind::Internal,
                format!("Nie wykonano: dziennik cofania: {e}."),
            )
        })?;
        match self.journal.snapshot_scope(step, &PathBuf::from(cwd)) {
            Ok(()) => Ok(step),
            Err(e) => {
                let _ = self.journal.abort_step(step);
                let mut out = ToolOutcome::denied(DenialReason::Policy, "polecenie powłoki");
                out.text = match e {
                    UndoError::SnapshotTooLarge { .. } => format!(
                        "{} — {e}; zawęź katalog roboczy (snapshot zakresu jest obowiązkowy).",
                        out.text
                    ),
                    other => format!("{} — brak snapshotu zakresu: {other}.", out.text),
                };
                Err(Box::new(out))
            }
        }
    }

    fn spec(&self, args: &RunArgs, cwd: &str) -> ExecSpec {
        let shell = args.shell.unwrap_or(self.config.default_shell);
        let (program, argv, raw) = self.config.invocation(shell, &args.command);
        let allow: Vec<&str> = self
            .config
            .env_allowlist
            .iter()
            .map(String::as_str)
            .collect();
        let timeout_s = args
            .timeout_s
            .unwrap_or(self.config.timeout_default_s)
            .clamp(1, self.config.timeout_max_s);
        ExecSpec {
            process: ProcessSpec {
                cmd: PathBuf::from(program),
                args: argv,
                cwd: PathBuf::from(cwd),
                integrity: Integrity::Medium,
                memory_limit_mb: Some(self.config.memory_limit_mb),
            },
            raw_args: raw,
            env: filter_env(self.base_env.iter().cloned(), &allow),
            timeout_ms: u64::from(timeout_s) * 1000,
            max_output_bytes: self.config.output_max_bytes,
        }
    }

    async fn execute(
        &self,
        spec: ExecSpec,
        ctx: &ToolCtx,
        command: &str,
    ) -> Result<ExecOutput, PlatformError> {
        let jobs = self.jobs.clone();
        let label = format!("shell: {}", text::truncate_chars(command, 40).0);
        let control = Arc::new(ExecControl::new().on_spawn(move |h| {
            if let Some(j) = &jobs {
                j.register_job(h, ProcessRole::Tool("tools-shell".into()), &label);
            }
        }));
        let flag = control.cancel_flag();
        let cancel = ctx.cancel.clone();
        let watcher = tokio::spawn(async move {
            cancel.cancelled().await;
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        let exec = self.exec.clone();
        let ctl = control.clone();
        let result = tokio::task::spawn_blocking(move || exec.run_captured(spec, &ctl))
            .await
            .unwrap_or_else(|e| Err(PlatformError::Io(format!("wątek polecenia: {e}"))));
        watcher.abort();
        if let (Some(j), Some(h)) = (&self.jobs, control.spawned()) {
            j.unregister_job(h);
        }
        result
    }

    /// `shell_run`.
    pub(crate) async fn run(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: RunArgs = parse_args(args)?;
        let command = a.command.trim();
        if command.is_empty()
            || command.chars().count() > MAX_COMMAND_CHARS
            || command.contains('\0')
        {
            return Err(fail(
                ToolErrorKind::InvalidArgs,
                format!("Polecenie puste albo dłuższe niż {MAX_COMMAND_CHARS} znaków."),
            ));
        }
        let cwd = self.cwd(a.cwd.as_deref(), ctx).await?;
        let auths = self.capabilities(ctx, m, &cwd, command).await?;
        if ctx.cancel.is_cancelled() {
            self.release(&auths).await;
            return Ok(ToolOutcome::cancelled("polecenie powłoki"));
        }
        let step = match self.snapshot(ctx, m, &cwd) {
            Ok(s) => s,
            Err(out) => {
                self.release(&auths).await;
                return Ok(*out);
            }
        };
        let spec = self.spec(
            &RunArgs {
                command: command.to_owned(),
                ..a
            },
            &cwd,
        );
        self.emit(
            EVENT_STARTED,
            Level::Info,
            serde_json::json!({ "cwd": cwd, "command": text::redact_secrets(command) }),
            ctx,
        )
        .await;
        let result = self.execute(spec, ctx, command).await;
        let approval = auths.iter().find_map(|a| a.approval);
        self.release(&auths).await;
        let undo = match self.journal.commit_step(step) {
            Ok(summary) => Some(UndoRef {
                service: UndoService::Journal,
                id: step.0,
                text: summary.text,
            }),
            Err(_) => None,
        };
        let out = match result {
            Ok(out) => out,
            Err(e) => {
                let _ = self.journal.abort_step(step);
                return Ok(ToolOutcome::failed(
                    ToolErrorKind::Io,
                    format!("Nie uruchomiono polecenia: {e}."),
                ));
            }
        };
        let mut result = self.outcome(out, undo, ctx).await;
        result.approval = approval;
        Ok(result)
    }

    async fn outcome(&self, out: ExecOutput, undo: Option<UndoRef>, ctx: &ToolCtx) -> ToolOutcome {
        let (termination, code) = match out.termination {
            ExecTermination::Exited(c) => (Termination::Exited, Some(c)),
            ExecTermination::TimedOut => (Termination::TimedOut, None),
            ExecTermination::Cancelled => (Termination::Cancelled, None),
            ExecTermination::Killed => (Termination::Killed, None),
        };
        let half = self.config.output_max_chars / 2;
        let render = |b: &[u8]| {
            text::truncate_chars(&text::redact_secrets(&String::from_utf8_lossy(b)), half)
        };
        let ((stdout, cut_out), (stderr, cut_err)) = (render(&out.stdout), render(&out.stderr));
        let truncated = out.truncated() || cut_out || cut_err;
        let payload = serde_json::json!({ "termination": termination, "exit_code": code, "elapsed_ms": out.elapsed_ms, "truncated": truncated });
        let event = if termination == Termination::Exited {
            EVENT_EXITED
        } else {
            EVENT_KILLED
        };
        self.emit(event, Level::Info, payload, ctx).await;
        report_untrusted(&self.gate, ctx, TaintSource::File).await;
        let head = match (termination, code) {
            (Termination::Exited, Some(c)) => format!(
                "Polecenie zakończone, kod wyjścia {c} ({} ms).",
                out.elapsed_ms
            ),
            (Termination::TimedOut, _) => {
                "Przekroczony limit czasu — zabito całe drzewo procesów.".to_owned()
            }
            (Termination::Killed, _) => "Polecenie zabite (kill-switch).".to_owned(),
            _ => "Polecenie anulowane — zabito całe drzewo procesów.".to_owned(),
        };
        let undo_note = if undo.is_some() {
            " Zmiany w katalogu roboczym można cofnąć."
        } else {
            ""
        };
        let body = format!("{head}{undo_note}\n[stdout]\n{stdout}\n[stderr]\n{stderr}");
        let data = RunOutput {
            termination,
            exit_code: code,
            stdout,
            stderr,
            stdout_bytes: out.stdout_total,
            stderr_bytes: out.stderr_total,
            truncated,
            elapsed_ms: out.elapsed_ms,
            undo_step: undo.as_ref().map(|u| u.id),
        };
        let mut result = ToolOutcome::ok(body, serde_json::to_value(data).unwrap_or_default())
            .untrusted(TaintSource::File);
        result.status = match termination {
            Termination::Exited => ToolStatus::Ok,
            Termination::TimedOut => ToolStatus::Failed {
                error: ToolErrorKind::Timeout,
            },
            Termination::Cancelled | Termination::Killed => ToolStatus::Cancelled,
        };
        result.undo = undo;
        result.truncated = truncated;
        result
    }
}
