//! Wykonanie `dev_git` i `dev_test`: katalog roboczy (deny-lista), zgoda Brokera, polecenie
//! przez `cmd.exe` w Job Object, wynik (stan repozytorium, podsumowanie testów).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use platform_contract::{
    BUILD_ENV_ALLOWLIST, DEFAULT_ENV_ALLOWLIST, ExecControl, ExecOutput, ExecSpec, ExecTermination,
    Integrity, PlatformError, ProcessSpec, filter_env, with_build_defaults,
};
use safety_broker_contract::{Capability, PathScope, TaintSource};
use tools_common_contract::{
    DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, action_request, base_facts,
    parse_args, report_untrusted, text,
};
use tools_dev_contract::{
    GitAction, GitArgs, Runner, TEST_TIMEOUT_DEFAULT_S, TEST_TIMEOUT_MAX_S, TestArgs, TestSummary,
    detect_runner, git_command, git_status, test_command, test_counts,
};
use tools_shell_contract::{ShellKind, ShellToolsConfig};
use watchdog_contract::ProcessRole;

use crate::Core;

type Step<T> = Result<T, Box<ToolOutcome>>;

/// Zmienne narzędzi programisty ponad listę bazową (toolchainy w profilu użytkownika).
const DEV_ENV: [&str; 9] = [
    "CARGO_HOME",
    "RUSTUP_HOME",
    "GOPATH",
    "GOROOT",
    "JAVA_HOME",
    "DOTNET_ROOT",
    "NODE_PATH",
    "VIRTUAL_ENV",
    "PYTHONPATH",
];
/// Limit wyjścia na strumień.
const MAX_OUTPUT: usize = 2 * 1024 * 1024;
/// Najdłuższy tekst dla modelu.
const TEXT_MAX: usize = 30_000;
/// Limit polecenia Git (s).
const GIT_TIMEOUT_S: u64 = 60;

fn fail(kind: ToolErrorKind, text: impl Into<String>) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::failed(kind, text))
}

impl Core {
    fn workdir(&self, ctx: &ToolCtx) -> Step<String> {
        let dir = ctx.workdir.clone().ok_or_else(|| {
            fail(
                ToolErrorKind::InvalidArgs,
                "Sesja nie ma katalogu roboczego — wybierz go dla tej rozmowy.",
            )
        })?;
        if self.deny.is_denied_path(&dir, &self.env) {
            return Err(Box::new(ToolOutcome::denied(
                DenialReason::DenyList,
                "narzędzia programisty w tym katalogu",
            )));
        }
        Ok(dir)
    }

    async fn authorize(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        cap: Capability,
        command: &str,
        action: &str,
    ) -> Step<tools_common_contract::Authorization> {
        let mut facts = base_facts(m, ctx);
        facts.command = Some(command.to_owned());
        let auth = self
            .gate
            .authorize(action_request(ctx, cap.clone(), facts), ctx)
            .await
            .map_err(|e| Box::new(e.into_outcome(action)))?;
        if let Err(e) = self.gate.verify(&auth, &cap, &ctx.holder) {
            self.gate.release(std::slice::from_ref(&auth)).await;
            return Err(Box::new(e.into_outcome(action)));
        }
        Ok(auth)
    }

    fn scope(&self, dir: &str) -> Step<PathScope> {
        PathScope::tree(dir, &self.env)
            .map_err(|e| fail(ToolErrorKind::InvalidArgs, format!("Katalog roboczy: {e}.")))
    }

    async fn execute(
        &self,
        dir: &str,
        command: &str,
        timeout_s: u64,
        ctx: &ToolCtx,
    ) -> Result<ExecOutput, PlatformError> {
        let (program, argv, raw) = ShellToolsConfig::default().invocation(ShellKind::Cmd, command);
        let allow: Vec<&str> = DEFAULT_ENV_ALLOWLIST
            .iter()
            .chain(DEV_ENV.iter())
            .chain(BUILD_ENV_ALLOWLIST.iter())
            .copied()
            .collect();
        let threads = std::thread::available_parallelism().map_or(2, std::num::NonZero::get);
        let spec = ExecSpec {
            process: ProcessSpec {
                cmd: PathBuf::from(program),
                args: argv,
                cwd: PathBuf::from(dir),
                integrity: Integrity::default(),
                memory_limit_mb: None,
            },
            raw_args: raw,
            env: with_build_defaults(filter_env(self.base_env.iter().cloned(), &allow), threads),
            timeout_ms: timeout_s * 1000,
            max_output_bytes: MAX_OUTPUT,
        };
        let jobs = self.jobs.clone();
        let label = format!("dev: {}", text::truncate_chars(command, 40).0);
        let control = Arc::new(ExecControl::new().on_spawn(move |h| {
            if let Some(j) = &jobs {
                j.register_job(h, ProcessRole::Tool("tools-dev".into()), &label);
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

    fn output_text(&self, out: &ExecOutput) -> String {
        let mut t = self.exec.decode_output(&out.stdout);
        let err = self.exec.decode_output(&out.stderr);
        if !err.trim().is_empty() {
            t.push_str("\n[stderr]\n");
            t.push_str(&err);
        }
        text::redact_secrets(&t)
    }

    /// `dev_git`.
    pub(crate) async fn git(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: GitArgs = parse_args(args)?;
        let dir = self.workdir(ctx)?;
        let command = git_command(&a).map_err(|e| fail(ToolErrorKind::InvalidArgs, e))?;
        let cap = Capability::FsRead(self.scope(&dir)?);
        let auth = self
            .authorize(ctx, m, cap, &command, "odczyt repozytorium Git")
            .await?;
        let out = self.execute(&dir, &command, GIT_TIMEOUT_S, ctx).await;
        self.gate.release(std::slice::from_ref(&auth)).await;
        let out = out.map_err(|e| fail(ToolErrorKind::Io, format!("Nie uruchomiono git: {e}.")))?;
        let body = self.output_text(&out);
        if !matches!(out.termination, ExecTermination::Exited(0)) {
            let hint = if body.contains("not a git repository") {
                " Katalog roboczy nie jest repozytorium Git."
            } else if body.contains("is not recognized") || body.contains("nie jest rozpoznawane") {
                " Git nie jest zainstalowany (np. `winget install Git.Git`)."
            } else {
                ""
            };
            return Err(fail(
                ToolErrorKind::Io,
                format!(
                    "git zakończył się błędem ({:?}).{hint}\n{}",
                    out.termination,
                    text::truncate_chars(&body, 2_000).0
                ),
            ));
        }
        report_untrusted(&self.gate, ctx, TaintSource::File).await;
        let data = if a.action == GitAction::Status {
            serde_json::to_value(git_status(&body)).unwrap_or_default()
        } else {
            serde_json::json!({"truncated": out.truncated()})
        };
        let (shown, cut) = text::truncate_chars(&body, TEXT_MAX);
        let mut o = ToolOutcome::ok(
            if shown.trim().is_empty() {
                "(brak wyniku)".into()
            } else {
                shown
            },
            data,
        )
        .untrusted(TaintSource::File);
        o.truncated = cut || out.truncated();
        o.approval = auth.approval;
        Ok(o)
    }

    fn detect(&self, dir: &str) -> Option<Runner> {
        let files: Vec<String> = std::fs::read_dir(dir)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        let pkg = std::fs::read_to_string(Path::new(dir).join("package.json")).ok();
        detect_runner(&files, pkg.as_deref())
    }

    /// `dev_test`.
    pub(crate) async fn test(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: TestArgs = parse_args(args)?;
        let dir = self.workdir(ctx)?;
        let runner = match a.runner.or_else(|| self.detect(&dir)) {
            Some(r) => r,
            None => {
                return Err(fail(
                    ToolErrorKind::NotFound,
                    "Nie rozpoznano rodzaju projektu (brak Cargo.toml, package.json ze skryptem test, go.mod, *.csproj, pytest) — podaj `runner`.",
                ));
            }
        };
        let command = test_command(runner, a.filter.as_deref());
        let cap = Capability::ShellExec(self.scope(&dir)?);
        let auth = self
            .authorize(ctx, m, cap, &command, "uruchomienie testów projektu")
            .await?;
        let timeout = a
            .timeout_s
            .unwrap_or(TEST_TIMEOUT_DEFAULT_S)
            .clamp(1, TEST_TIMEOUT_MAX_S);
        let out = self.execute(&dir, &command, u64::from(timeout), ctx).await;
        self.gate.release(std::slice::from_ref(&auth)).await;
        let out =
            out.map_err(|e| fail(ToolErrorKind::Io, format!("Nie uruchomiono testów: {e}.")))?;
        if out.termination == ExecTermination::Cancelled {
            return Ok(ToolOutcome::cancelled("uruchomienie testów"));
        }
        let body = self.output_text(&out);
        let counts = test_counts(runner, &body);
        let exit_code = match out.termination {
            ExecTermination::Exited(c) => Some(c),
            _ => None,
        };
        let summary = TestSummary {
            runner,
            command,
            passed: counts.passed,
            failed: counts.failed,
            skipped: counts.skipped,
            failures: counts.failures,
            exit_code,
            timed_out: out.termination == ExecTermination::TimedOut,
            ok: exit_code == Some(0) && counts.failed == 0,
            elapsed_ms: out.elapsed_ms,
        };
        report_untrusted(&self.gate, ctx, TaintSource::File).await;
        let mut head = if summary.timed_out {
            format!("Testy przerwane po {timeout} s (limit czasu).")
        } else if !counts.found && !summary.ok {
            "Testy nie wystartowały albo zakończyły się błędem przed podsumowaniem (np. błąd kompilacji) — szczegóły niżej.".into()
        } else {
            format!(
                "{}: zaliczone {}, niezaliczone {}, pominięte {} ({:.1} s).",
                if summary.ok {
                    "Testy zaliczone"
                } else {
                    "Testy z błędami"
                },
                summary.passed,
                summary.failed,
                summary.skipped,
                summary.elapsed_ms as f64 / 1000.0
            )
        };
        if !summary.failures.is_empty() {
            head.push_str(&format!("\nNiezaliczone: {}", summary.failures.join(", ")));
        }
        // Koniec wyjścia jest najważniejszy (podsumowanie, błędy kompilacji).
        let tail: String = {
            let chars: Vec<char> = body.chars().collect();
            chars[chars.len().saturating_sub(TEXT_MAX)..]
                .iter()
                .collect()
        };
        let mut o = ToolOutcome::ok(
            format!("{head}\n\nWyjście (koniec):\n{tail}"),
            serde_json::to_value(&summary).unwrap_or_default(),
        )
        .untrusted(TaintSource::File);
        o.truncated = out.truncated() || tail.len() < body.len();
        o.approval = auth.approval;
        Ok(o)
    }
}
