//! Wersje i flagi CLI programów z katalogu (`--version`, `--help`) — inna wersja wydania wyjdzie
//! w próbie generalnej, nie u właściciela.

use std::path::Path;
use std::time::Duration;

use app_api::AppPaths;
use serde_json::{Value, json};

use super::Report;

/// Uruchamia program z argumentami (limit czasu); zwraca (kod wyjścia, stdout+stderr).
pub async fn run(program: &Path, args: &[&str], limit: Duration) -> (Option<i32>, String) {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    match tokio::time::timeout(limit, cmd.output()).await {
        Ok(Ok(out)) => {
            let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&out.stderr));
            (out.status.code(), text)
        }
        Ok(Err(e)) => (None, format!("nie uruchomiono: {e}")),
        Err(_) => (None, "przekroczony czas".into()),
    }
}

/// Wersja i flagi CLI używane przez Alfę, sprawdzone w `--help` programu.
pub async fn probe_cli(program: &Path, version_args: &[&str], flags: &[&str]) -> Value {
    let (vcode, version) = if version_args.is_empty() {
        (None, String::new())
    } else {
        run(program, version_args, Duration::from_secs(30)).await
    };
    let (hcode, help) = run(program, &["--help"], Duration::from_secs(30)).await;
    let missing: Vec<&str> = flags
        .iter()
        .copied()
        .filter(|f| {
            !help
                .split(|c: char| c.is_whitespace() || c == ',')
                .any(|w| w == *f)
        })
        .collect();
    json!({
        "program": program.display().to_string(),
        "version_exit": vcode,
        "version": version.lines().take(8).collect::<Vec<_>>(),
        "help_exit": hcode,
        "help_head": help.lines().take(5).collect::<Vec<_>>(),
        "flags_missing_in_help": missing,
    })
}

/// Wersje i flagi CLI (`--help`), których używa Alfa — inna wersja wydania wyjdzie tu, nie u
/// właściciela. Programy GPU na runnerze bez GPU mogą nie wystartować — tylko do raportu.
pub async fn probe_programs(paths: &AppPaths, report: &mut Report) {
    let llama_flags = [
        "--host",
        "--port",
        "--api-key",
        "-m",
        "--alias",
        "-c",
        "-ngl",
        "--threads",
        "-np",
        "--jinja",
    ];
    let whisper_flags = ["-m", "--host", "--port", "-t", "-nlp", "-sns", "-ng", "-fa"];
    let mut out = serde_json::Map::new();
    for (dir, file, version, flags, required) in [
        (
            "llama-cpu",
            "llama-server",
            &["--version"][..],
            &llama_flags[..],
            true,
        ),
        (
            "llama-vulkan",
            "llama-server",
            &["--version"],
            &llama_flags,
            false,
        ),
        (
            "llama-cuda",
            "llama-server",
            &["--version"],
            &llama_flags,
            false,
        ),
        ("whisper", "whisper-server", &[], &whisper_flags, true),
        ("whisper-cuda", "whisper-server", &[], &whisper_flags, false),
        (
            "piper",
            "piper",
            &["--version"],
            &["--model", "--output_raw", "--quiet"],
            true,
        ),
    ] {
        let program = paths.sidecar(dir, file);
        if !program.is_file() {
            continue;
        }
        let probe = probe_cli(&program, version, flags).await;
        let missing = probe["flags_missing_in_help"]
            .as_array()
            .map_or(0, Vec::len);
        if required && missing > 0 {
            report.problem(format!(
                "{dir}/{file}: brak flag w --help: {}",
                probe["flags_missing_in_help"]
            ));
        }
        out.insert(dir.to_owned(), probe);
    }
    report.set("programs", serde_json::Value::Object(out));
}
