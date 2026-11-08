//! [`ProcessEngine`] — port silnika nad prawdziwym `engine-server.exe`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Child;
use tools_engine_contract::{
    Capture, DocEntry, EngineError, EngineMethod, EnginePort, EngineState, Installation, Listening,
    PreviewOptions, SERVER_EXE, StartOptions,
};

use crate::conn::Connection;
use crate::{files, process};

/// Czekanie na adres serwera (okno podglądu inicjuje kartę graficzną dłużej).
const START_TIMEOUT: Duration = Duration::from_secs(60);
/// Zwykłe wywołanie.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// Kroki symulacji i transakcje (do 10 000 klatek / 1024 operacji).
const LONG_TIMEOUT: Duration = Duration::from_secs(180);
/// Ponad czas klatek podglądu (inicjacja karty, zapis PNG).
const PREVIEW_MARGIN: Duration = Duration::from_secs(90);

/// Katalog silnika z Ustawień (aplikacja odświeża go po zmianie ustawienia).
pub type EngineRoot = Arc<RwLock<Option<PathBuf>>>;

struct Live {
    child: Child,
    state: EngineState,
    captures: Option<PathBuf>,
    tail: process::Tail,
}

struct Inner {
    root: EngineRoot,
    data: PathBuf,
    base_env: Vec<(String, String)>,
    live: Mutex<Option<Live>>,
    conn: tokio::sync::Mutex<Option<Connection>>,
    previews: Mutex<Vec<Child>>,
    /// Licznik użyć kill-switcha: start, który czekał na adres serwera, gdy kill-switch zadziałał,
    /// nie zapisuje procesu jako działającego, tylko go kończy (przegląd 2026-10-27).
    kills: AtomicU64,
}

/// Port silnika nad procesem serwera.
#[derive(Clone)]
pub struct ProcessEngine {
    inner: Arc<Inner>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl ProcessEngine {
    /// Port: katalog silnika z Ustawień, katalog danych Alfy na zrzuty i podglądy
    /// (`<dane>/engine`), środowisko bazowe procesów (filtrowane listą dozwoloną).
    pub fn new(root: EngineRoot, data: PathBuf, base_env: Vec<(String, String)>) -> Self {
        Self {
            inner: Arc::new(Inner {
                root,
                data,
                base_env,
                live: Mutex::new(None),
                conn: tokio::sync::Mutex::new(None),
                previews: Mutex::new(Vec::new()),
                kills: AtomicU64::new(0),
            }),
        }
    }

    fn root(&self) -> Result<PathBuf, EngineError> {
        self.inner
            .root
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or(EngineError::NotConfigured)
    }

    fn server(&self) -> Result<PathBuf, EngineError> {
        let exe = self.root()?.join(SERVER_EXE);
        if exe.is_file() {
            Ok(exe)
        } else {
            Err(EngineError::NotInstalled(exe.display().to_string()))
        }
    }

    fn workdir(&self) -> Result<PathBuf, EngineError> {
        std::fs::create_dir_all(&self.inner.data)
            .map_err(|e| EngineError::Io(format!("{}: {e}", self.inner.data.display())))?;
        Ok(self.inner.data.clone())
    }

    /// Sprząta po procesie, który zakończył się sam (awaria, zamknięte okno podglądu).
    fn reap(&self) -> Option<i32> {
        let mut live = lock(&self.inner.live);
        let code = live.as_mut().and_then(|l| process::exited(&mut l.child))?;
        *live = None;
        code.or(Some(-1))
    }

    fn scene_label(options: &StartOptions) -> String {
        match &options.load {
            Some(p) => p.display().to_string(),
            None => serde_json::to_value(options.scene)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
        }
    }

    fn timeout(method: EngineMethod) -> Duration {
        match method {
            EngineMethod::HeadlessStep
            | EngineMethod::ExecuteTransaction
            | EngineMethod::HotReloadShader
            | EngineMethod::StreamWorld => LONG_TIMEOUT,
            _ => CALL_TIMEOUT,
        }
    }
}

fn check_load(load: Option<&Path>) -> Result<(), EngineError> {
    let Some(path) = load else { return Ok(()) };
    let meta =
        std::fs::metadata(path).map_err(|e| EngineError::Io(format!("{}: {e}", path.display())))?;
    if !meta.is_file() || meta.len() > 64 * 1024 * 1024 {
        return Err(EngineError::Io(format!(
            "{}: zapis świata musi być plikiem ≤ 64 MiB",
            path.display()
        )));
    }
    Ok(())
}

#[async_trait]
impl EnginePort for ProcessEngine {
    fn installation(&self) -> Installation {
        files::installation(self.root().ok().as_deref())
    }

    fn state(&self) -> EngineState {
        if let Some(code) = self.reap() {
            return EngineState {
                exit_code: Some(code),
                ..EngineState::default()
            };
        }
        lock(&self.inner.live)
            .as_ref()
            .map(|l| l.state.clone())
            .unwrap_or_default()
    }

    async fn start(&self, options: StartOptions) -> Result<EngineState, EngineError> {
        let mut conn = self.inner.conn.lock().await;
        self.reap();
        if let Some(l) = lock(&self.inner.live).as_ref() {
            return Err(EngineError::AlreadyRunning(
                l.state.url.clone().unwrap_or_default(),
            ));
        }
        let program = self.server()?;
        check_load(options.load.as_deref())?;
        let workdir = self.workdir()?;
        let captures = if options.window {
            Some(process::fresh_dir(&workdir, "captures")?)
        } else {
            None
        };
        let args = process::server_args(&options, captures.as_deref());
        let kills = self.inner.kills.load(Ordering::SeqCst);
        let spawned = process::spawn(&program, &args, &workdir, &self.inner.base_env)?;
        let (mut child, tail) = (spawned.child, spawned.tail);
        let opened = async {
            let line = process::first_line(spawned.stdout, START_TIMEOUT).await?;
            let listening = Listening::parse(&line)?;
            let c = Connection::open(&listening.host_port()?).await?;
            Ok::<_, EngineError>((listening, c))
        }
        .await;
        let opened = opened.and_then(|v| {
            if self.inner.kills.load(Ordering::SeqCst) == kills {
                Ok(v)
            } else {
                Err(EngineError::Spawn(
                    "przerwane wyłącznikiem awaryjnym".into(),
                ))
            }
        });
        let (listening, c) = match opened {
            Ok(v) => v,
            Err(e) => {
                process::kill(&mut child);
                // Chwila na dopisanie przyczyny przez proces (np. zajęty port, brak karty).
                tokio::time::sleep(Duration::from_millis(200)).await;
                let why = tail.text();
                return Err(match (e, why.is_empty()) {
                    (e, true) => e,
                    (_, false) => EngineError::Spawn(why),
                });
            }
        };
        let state = EngineState {
            running: true,
            url: Some(listening.listening.clone()),
            mode: Some(listening.mode.clone()),
            paused: listening.paused,
            window: options.window,
            pid: child.id(),
            scene: Some(Self::scene_label(&options)),
            exit_code: None,
        };
        *conn = Some(c);
        *lock(&self.inner.live) = Some(Live {
            child,
            state: state.clone(),
            captures,
            tail,
        });
        Ok(state)
    }

    async fn stop(&self) -> Result<bool, EngineError> {
        let mut conn = self.inner.conn.lock().await;
        if let Some(mut c) = conn.take() {
            c.close().await;
        }
        let Some(mut live) = lock(&self.inner.live).take() else {
            return Ok(false);
        };
        process::kill(&mut live.child);
        // Proces kończy się od razu; czekamy krótko, żeby port i pliki były wolne.
        let _ = tokio::time::timeout(Duration::from_secs(5), live.child.wait()).await;
        Ok(true)
    }

    async fn call(&self, method: EngineMethod, params: Value) -> Result<Value, EngineError> {
        let mut conn = self.inner.conn.lock().await;
        if let Some(code) = self.reap() {
            *conn = None;
            return Err(EngineError::Spawn(format!(
                "serwer silnika zakończył się (kod {code}) — uruchom go ponownie"
            )));
        }
        // Kill-switch zabrał proces — połączenie jest martwe.
        let Some(tail) = lock(&self.inner.live).as_ref().map(|l| l.tail.clone()) else {
            *conn = None;
            return Err(EngineError::NotRunning);
        };
        let c = conn.as_mut().ok_or(EngineError::NotRunning)?;
        let result = c.call(method, &params, Self::timeout(method)).await;
        if let Err(EngineError::Transport(_)) = &result
            && let Some(code) = self.reap()
        {
            *conn = None;
            return Err(EngineError::Spawn(format!(
                "serwer silnika przestał działać w trakcie wywołania (kod {code}): {}",
                tail.text()
            )));
        }
        result
    }

    async fn capture(&self) -> Result<Option<Capture>, EngineError> {
        let dir = lock(&self.inner.live)
            .as_ref()
            .ok_or(EngineError::NotRunning)?
            .captures
            .clone();
        match dir {
            Some(d) => files::latest_capture(&d),
            None => Ok(None),
        }
    }

    async fn preview(&self, options: PreviewOptions) -> Result<Capture, EngineError> {
        // Podglądy porzucone przez anulowanie kończą się same — sprzątamy ich wpisy.
        lock(&self.inner.previews).retain_mut(|c| process::exited(c).is_none());
        let program = self.server()?;
        check_load(options.load.as_deref())?;
        let workdir = self.workdir()?;
        let out = process::fresh_dir(&workdir, "previews")?;
        let args = process::preview_args(&options, &out);
        let spawned = process::spawn(&program, &args, &workdir, &self.inner.base_env)?;
        let tail = spawned.tail.clone();
        let pid = spawned.child.id();
        // Wyjście czytane w tle (potok nie może się zapełnić); proces w rejestrze kill-switcha.
        tokio::spawn(process::first_line(spawned.stdout, PREVIEW_MARGIN));
        lock(&self.inner.previews).push(spawned.child);
        let limit = Duration::from_millis(u64::from(options.frames) * 1000 / 30) + PREVIEW_MARGIN;
        let started = std::time::Instant::now();
        let code = loop {
            let finished = {
                let mut previews = lock(&self.inner.previews);
                let at = previews.iter().position(|c| c.id() == pid);
                match at {
                    None => Some(None),
                    Some(i) => process::exited(&mut previews[i]).inspect(|_| {
                        previews.remove(i);
                    }),
                }
            };
            if let Some(code) = finished {
                break code;
            }
            if started.elapsed() > limit {
                let mut previews = lock(&self.inner.previews);
                if let Some(i) = previews.iter().position(|c| c.id() == pid) {
                    let mut child = previews.remove(i);
                    process::kill(&mut child);
                }
                return Err(EngineError::Timeout(limit.as_secs()));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        files::preview_capture(&out).map_err(|e| match code {
            Some(0) => e,
            _ => EngineError::Spawn(format!("podgląd zakończył się błędem: {}", tail.text())),
        })
    }

    async fn docs(&self) -> Result<Vec<DocEntry>, EngineError> {
        let root = self.root()?;
        tokio::task::spawn_blocking(move || files::docs(&root))
            .await
            .map_err(|e| EngineError::Io(e.to_string()))?
    }

    async fn read_doc(&self, name: &str) -> Result<String, EngineError> {
        let (root, name) = (self.root()?, name.to_owned());
        tokio::task::spawn_blocking(move || files::read_doc(&root, &name))
            .await
            .map_err(|e| EngineError::Io(e.to_string()))?
    }

    fn kill_all(&self) -> usize {
        self.inner.kills.fetch_add(1, Ordering::SeqCst);
        let mut killed = 0;
        if let Some(mut live) = lock(&self.inner.live).take() {
            process::kill(&mut live.child);
            killed += 1;
        }
        for mut child in lock(&self.inner.previews).drain(..) {
            process::kill(&mut child);
            killed += 1;
        }
        killed
    }
}
