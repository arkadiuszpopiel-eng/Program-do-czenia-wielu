//! `shell_run`: analiza polecenia → deny-lista i polityka (bez Brokera) → zgody Brokera
//! (`shell.exec(cwd)` z poleceniem do reguł Jądra, zakresy ścieżek spoza `cwd`, `net.egress`
//! dla hostów) → snapshot zakresu (brak snapshotu = brak wykonania) → `ExecPort` w Job Object
//! (rejestr kill-switcha, anulowanie) → zatwierdzenie kroku → wyjście zredagowane i niezaufane.

use std::path::PathBuf;
use std::sync::Arc;

use core_bus_contract::Level;
use platform_contract::{
    ExecControl, ExecOutput, ExecSpec, Integrity, PlatformError, ProcessSpec, filter_env,
};
use risk_classifier_contract::Reversibility;
use safety_broker_contract::{Capability, HostPattern};
use tools_common_contract::{
    Authorization, DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, UndoRef,
    UndoService, base_facts, parse_args, paths, text,
};
use tools_shell_contract::{EVENT_DENIED, EVENT_STARTED, MAX_COMMAND_CHARS, RunArgs, analyze};
use undo_journal_contract::{StepCtx, StepId, UndoError};
use watchdog_contract::ProcessRole;

use crate::Core;
use crate::isolation::{Isolation, uses_profile_toolchain};

type Step<T> = Result<T, Box<ToolOutcome>>;

fn fail(kind: ToolErrorKind, text: String) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::failed(kind, text))
}

/// Cel ścieżki z polecenia względem `cwd`: zmienne rozwinięte, `..` zwinięte, wieloznacznik
/// w ostatnim segmencie → jego katalog. `None` = celu nie da się ustalić (nieznana zmienna,
/// wieloznacznik wyżej, `..` ponad korzeń) — wtedy usuwanie jest nieodwracalne (Q-8).
fn target_path(raw: &str, cwd: &str, env: &compliance_contract::PathEnv) -> Option<String> {
    let trimmed = raw.trim_end_matches(['/', '\\']);
    let dir = match trimmed.rsplit_once(['/', '\\']) {
        Some((parent, last)) if last.contains(['*', '?']) => parent,
        _ => trimmed,
    };
    paths::resolve_path_dots(dir, Some(cwd), env).ok()
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
    ) -> Step<(Vec<Authorization>, bool)> {
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
        let mut unresolved = false;
        for raw in &a.paths {
            let Some(resolved) = target_path(raw, cwd, &self.env) else {
                unresolved = true;
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
        facts.destructive = a.destructiveness(outside.is_empty() && !unresolved);
        if a.deletes && (a.opaque_targets || unresolved) {
            // Cel usuwania nieustalony: najgorszy przypadek — trwałe i masowe (zgoda do L3).
            facts.reversible = Reversibility::No;
            facts.bulk = u32::MAX;
        }
        facts.install = a.install;
        facts.command = Some(command.to_owned());
        // Izolacja (integralność Low): tylko gdy całe znane działanie mieści się w katalogu
        // roboczym — instalacje, sieć i cele spoza katalogu idą jak dotąd, za zgodą Brokera.
        let isolate = self.config.low_integrity
            && outside.is_empty()
            && !unresolved
            && a.hosts.is_empty()
            && !a.network
            && !a.install
            && !a.opaque_targets
            && !uses_profile_toolchain(command);
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
        Ok((self.authorize(ctx, caps, &action).await?, isolate))
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

    fn spec(&self, args: &RunArgs, cwd: &str, integrity: Integrity) -> ExecSpec {
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
                integrity,
                memory_limit_mb: Some(self.config.memory_limit_mb),
            },
            raw_args: raw,
            // `cargo` z powłoki też z limitem równoległych zadań (awaria 2026-10-17).
            env: platform_contract::with_build_defaults(
                filter_env(self.base_env.iter().cloned(), &allow),
                std::thread::available_parallelism().map_or(2, std::num::NonZero::get),
            ),
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
        let (auths, isolate) = self.capabilities(ctx, m, &cwd, command).await?;
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
        let integrity = if isolate {
            Integrity::Low
        } else {
            Integrity::Medium
        };
        let spec = self.spec(
            &RunArgs {
                command: command.to_owned(),
                ..a
            },
            &cwd,
            integrity,
        );
        self.emit(
            EVENT_STARTED,
            Level::Info,
            serde_json::json!({ "cwd": cwd, "command": text::redact_secrets(command) }),
            ctx,
        )
        .await;
        let mut isolation = if isolate {
            Isolation::Low
        } else {
            Isolation::Off
        };
        let mut result = self.execute(spec.clone(), ctx, command).await;
        if isolate && let Err(PlatformError::Unsupported(why)) = &result {
            // Izolacji nie da się zastosować (katalog zbyt szeroki, dysk bez ACL — exFAT, udział
            // sieciowy): polecenie i tak ma zgody na działanie w katalogu, więc idzie jak dotąd.
            isolation = Isolation::Unavailable(why.clone());
            let mut medium = spec;
            medium.process.integrity = Integrity::Medium;
            result = self.execute(medium, ctx, command).await;
        }
        let approval = auths.iter().find_map(|a| a.approval);
        self.release(&auths).await;
        // Krok trafia do dziennika dopiero po uruchomieniu polecenia: nieudany start zostawiał
        // wcześniej pusty, zatwierdzony krok „do cofnięcia".
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
        let undo = match self.journal.commit_step(step) {
            Ok(summary) => Some(UndoRef {
                service: UndoService::Journal,
                id: step.0,
                text: summary.text,
            }),
            Err(_) => None,
        };
        let mut result = self.outcome(out, undo, ctx).await;
        match isolation {
            Isolation::Low => result.text.push_str(
                "
[izolacja] Polecenie działało z niską integralnością: zapis tylko w katalogu roboczym (odmowa dostępu poza nim to zabezpieczenie, nie usterka).",
            ),
            Isolation::Unavailable(why) => result.text.push_str(&format!(
                "
[izolacja] Niedostępna ({why}) — polecenie działało jak zwykle, w granicach zgód."
            )),
            Isolation::Off => {}
        }
        result.approval = approval;
        Ok(result)
    }
}
