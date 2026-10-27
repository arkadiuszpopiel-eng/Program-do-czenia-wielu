//! Proces `engine-server.exe`: argumenty, środowisko bez sekretów, bez okna konsoli, rejestr
//! procesów pobocznych (sprzątanie po awarii Alfy), ogon stderr do komunikatów błędów.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use platform_contract::{DEFAULT_ENV_ALLOWLIST, filter_env};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdout, Command};
use tools_engine_contract::{EngineError, PreviewOptions, Scene, StartOptions};

/// Zmienne dla sterowników grafiki i dźwięku ponad listę bazową.
const ENGINE_ENV: [&str; 6] = [
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
    "WGPU_BACKEND",
    "WGPU_ADAPTER_NAME",
    "RUST_LOG",
    "RUST_BACKTRACE",
];
/// Ile ostatnich wierszy stderr pamiętamy.
const STDERR_TAIL: usize = 20;

/// Flaga `CREATE_NO_WINDOW` (bez okna konsoli; okno podglądu silnik tworzy sam).
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Ostatnie wiersze stderr procesu.
#[derive(Clone, Default)]
pub(crate) struct Tail(Arc<Mutex<VecDeque<String>>>);

impl Tail {
    fn push(&self, line: String) {
        let mut q = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if q.len() == STDERR_TAIL {
            q.pop_front();
        }
        q.push_back(line);
    }

    /// Ostatnie wiersze (do komunikatu błędu).
    pub(crate) fn text(&self) -> String {
        let q = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        q.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

fn scene_args(args: &mut Vec<String>, scene: Scene, load: Option<&Path>) {
    if let Some(path) = load {
        args.push("--load".into());
        args.push(path.display().to_string());
        return;
    }
    match scene {
        Scene::Empty => {}
        Scene::Demo => args.push("--demo".into()),
        Scene::Industrial => args.push("--industrial".into()),
    }
}

/// Argumenty serwera dla [`StartOptions`] (`captures` — katalog zrzutów okna podglądu).
pub(crate) fn server_args(options: &StartOptions, captures: Option<&Path>) -> Vec<String> {
    let mut args = vec!["--listen".into(), "127.0.0.1:0".into()];
    // Zrzuty rewizji wymagają `--paused` (warunek serwera).
    if options.paused || options.window {
        args.push("--paused".into());
    }
    if options.audio {
        args.push("--audio".into());
    }
    if let (true, Some(dir)) = (options.window, captures) {
        args.extend([
            "--preview".into(),
            "--capture-revisions".into(),
            "--preview-output".into(),
            dir.display().to_string(),
        ]);
    }
    scene_args(&mut args, options.scene, options.load.as_deref());
    args
}

/// Argumenty jednorazowego podglądu.
pub(crate) fn preview_args(options: &PreviewOptions, out: &Path) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "--listen".into(),
        "127.0.0.1:0".into(),
        "--preview".into(),
        "--frames".into(),
        options.frames.to_string(),
        "--width".into(),
        options.width.to_string(),
        "--height".into(),
        options.height.to_string(),
        "--preview-output".into(),
        out.display().to_string(),
    ];
    scene_args(&mut args, options.scene, options.load.as_deref());
    args
}

/// Uruchomiony proces z wyjściem i ogonem stderr.
pub(crate) struct Spawned {
    pub(crate) child: Child,
    pub(crate) stdout: ChildStdout,
    pub(crate) tail: Tail,
}

/// Uruchamia program silnika: katalog roboczy w danych Alfy (nic nie powstaje w katalogu
/// silnika), środowisko z listy dozwolonej, stdin zamknięty.
pub(crate) fn spawn(
    program: &Path,
    args: &[String],
    cwd: &Path,
    base_env: &[(String, String)],
) -> Result<Spawned, EngineError> {
    let allow: Vec<&str> = DEFAULT_ENV_ALLOWLIST
        .iter()
        .chain(ENGINE_ENV.iter())
        .copied()
        .collect();
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(filter_env(base_env.iter().cloned(), &allow))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let mut child = cmd
        .spawn()
        .map_err(|e| EngineError::Spawn(format!("{}: {e}", program.display())))?;
    if let Some(pid) = child.id() {
        lib_sidecar_guard::register(pid, program);
    }
    let tail = Tail::default();
    if let Some(stderr) = child.stderr.take() {
        let tail = tail.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!(target: "engine_server", "{line}");
                tail.push(line);
            }
        });
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| EngineError::Spawn("brak wyjścia procesu".into()))?;
    Ok(Spawned {
        child,
        stdout,
        tail,
    })
}

/// Czeka na pierwszy wiersz stdout (adres); potem dalej czyta wyjście w tle, żeby proces nie
/// zatrzymał się na pełnym potoku.
pub(crate) async fn first_line(
    stdout: ChildStdout,
    limit: Duration,
) -> Result<String, EngineError> {
    let mut lines = BufReader::new(stdout).lines();
    let line = tokio::time::timeout(limit, lines.next_line())
        .await
        .map_err(|_| EngineError::Timeout(limit.as_secs()))?
        .map_err(|e| EngineError::Spawn(e.to_string()))?
        .ok_or_else(|| EngineError::Spawn("serwer zakończył się bez adresu".into()))?;
    tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
    Ok(line)
}

/// Kończy proces (bez czekania) i wyrejestrowuje go.
pub(crate) fn kill(child: &mut Child) {
    if let Some(pid) = child.id() {
        lib_sidecar_guard::unregister(pid);
    }
    // Błąd = proces już nie żyje.
    let _ = child.start_kill();
}

/// Kod wyjścia, gdy proces już się zakończył.
pub(crate) fn exited(child: &mut Child) -> Option<Option<i32>> {
    match child.try_wait() {
        Ok(Some(status)) => {
            if let Some(pid) = child.id() {
                lib_sidecar_guard::unregister(pid);
            }
            Some(status.code())
        }
        Ok(None) => None,
        Err(_) => Some(None),
    }
}

/// Katalog z datą w danych Alfy (`<dane>/engine/<rodzaj>/<data>-<n>`).
pub(crate) fn fresh_dir(base: &Path, kind: &str) -> Result<PathBuf, EngineError> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let dir = base.join(kind).join(format!("{stamp}"));
    std::fs::create_dir_all(&dir)
        .map_err(|e| EngineError::Io(format!("{}: {e}", dir.display())))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_match_server_rules() {
        let window = StartOptions {
            scene: Scene::Demo,
            window: true,
            ..StartOptions::default()
        };
        let args = server_args(&window, Some(Path::new("C:\\d")));
        assert_eq!(&args[..3], ["--listen", "127.0.0.1:0", "--paused"]);
        assert!(args.contains(&"--capture-revisions".to_owned()));
        assert!(args.ends_with(&["--demo".to_owned()]));
        let load = StartOptions {
            load: Some(PathBuf::from("C:\\w\\s.json")),
            audio: true,
            ..StartOptions::default()
        };
        let args = server_args(&load, None);
        assert!(!args.contains(&"--paused".to_owned()));
        assert!(args.contains(&"--audio".to_owned()));
        assert!(args.ends_with(&["--load".to_owned(), "C:\\w\\s.json".to_owned()]));
        assert!(!args.contains(&"--preview".to_owned()));
        let preview = PreviewOptions {
            scene: Scene::Industrial,
            frames: 3,
            width: 640,
            height: 360,
            load: None,
        };
        let args = preview_args(&preview, Path::new("C:\\p"));
        assert!(args.windows(2).any(|w| w == ["--frames", "3"]));
        assert!(args.windows(2).any(|w| w == ["--width", "640"]));
        assert!(args.ends_with(&["--industrial".to_owned()]));
        assert!(
            !args.iter().any(|a| a.contains("9001")),
            "nigdy domyślny port silnika"
        );
    }

    #[test]
    fn tail_keeps_last_lines() {
        let t = Tail::default();
        for i in 0..30 {
            t.push(format!("w{i}"));
        }
        let text = t.text();
        assert!(text.starts_with("w10") && text.ends_with("w29"));
    }
}
