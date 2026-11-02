//! Wynik `shell_run` dla modelu: zakończenie, kod wyjścia, wyjście zredagowane i niezaufane,
//! krok do cofnięcia.

use core_bus_contract::Level;
use platform_contract::{ExecOutput, ExecTermination};
use safety_broker_contract::TaintSource;
use tools_common_contract::{
    ToolCtx, ToolErrorKind, ToolOutcome, ToolStatus, UndoRef, report_untrusted, text,
};
use tools_shell_contract::{EVENT_EXITED, EVENT_KILLED, RunOutput, Termination};

use crate::Core;

impl Core {
    pub(crate) async fn outcome(
        &self,
        out: ExecOutput,
        undo: Option<UndoRef>,
        ctx: &ToolCtx,
    ) -> ToolOutcome {
        let (termination, code) = match out.termination {
            ExecTermination::Exited(c) => (Termination::Exited, Some(c)),
            ExecTermination::TimedOut => (Termination::TimedOut, None),
            ExecTermination::Cancelled => (Termination::Cancelled, None),
            ExecTermination::Killed => (Termination::Killed, None),
        };
        let half = self.config.output_max_chars / 2;
        // Kodowanie konsoli zna warstwa platformy (`cmd.exe` pisze w stronie kodowej OEM).
        let render = |b: &[u8]| {
            text::truncate_chars(&text::redact_secrets(&self.exec.decode_output(b)), half)
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
            " Zmiany w katalogu roboczym można cofnąć (bez katalogów zależności i wyników \
             budowania: node_modules, __pycache__, .venv, target cargo)."
        } else {
            ""
        };
        let body = format!(
            "{head}{undo_note}\n[stdout]\n{stdout}\n[stderr]\n{stderr}{}",
            hint(&stderr)
        );
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

/// Podpowiedź po typowej porażce środowiska — model powtarzał to samo polecenie (pomiar F3 sh-04,
/// sh-10, fs-23, 2.11): narzędzia uniksowe z Git padają w izolacji niskiej integralności, a polecenie
/// PowerShell w `cmd` „nie jest rozpoznawane”.
pub(crate) fn hint(stderr: &str) -> &'static str {
    if stderr.contains("NtCreateDirectoryObject") {
        "\n[podpowiedź] Narzędzia uniksowe z Git (grep, rm, sed, tr…) nie działają w izolacji polecenia — \
         użyj poleceń PowerShell (Select-String, Get-Content, Remove-Item, Measure-Object) albo narzędzi \
         plików (fs_*)."
    } else if stderr.contains("internal or external command")
        || stderr.contains("polecenie wewnętrzne lub zewnętrzne")
    {
        // Komunikat `cmd.exe`: polecenie PowerShell uruchomione w `cmd`.
        "\n[podpowiedź] `cmd` nie zna tego polecenia — polecenia PowerShell (Get-ChildItem, \
         Select-String…) uruchamiaj z `shell`: „powershell”, a zamiast narzędzi uniksowych użyj ich \
         odpowiedników PowerShell albo narzędzi plików (fs_*)."
    } else if stderr.contains("is not recognized as") || stderr.contains("nie jest rozpoznawan") {
        // Komunikat PowerShell: zwykle narzędzie uniksowe (grep, sed) — zmiana powłoki nie pomoże.
        "\n[podpowiedź] Tego polecenia nie ma na tym komputerze — użyj odpowiednika PowerShell \
         (Select-String, Get-Content, Remove-Item, Measure-Object) albo narzędzi plików (fs_*)."
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::hint;

    #[test]
    fn environment_failures_get_a_hint() {
        let msys =
            "0 [main] grep (30204) grep.exe: *** fatal error - NtCreateDirectoryObject: 0xC0000022";
        assert!(hint(msys).contains("Select-String"));
        let cmd = "'Get-ChildItem' is not recognized as an internal or external command,";
        assert!(hint(cmd).contains("„powershell”"));
        assert!(hint("'grep' nie jest rozpoznawany jako polecenie wewnętrzne").contains("fs_*"));
        // PowerShell: bez rady o zmianie powłoki (model już jest w PowerShell).
        let pwsh = "The term 'grep' is not recognized as the name of a cmdlet, function";
        assert!(hint(pwsh).contains("Select-String") && !hint(pwsh).contains("„powershell”"));
        assert_eq!(hint("Access is denied."), "");
    }
}
