//! `tools-engine` — atrapa (docs/modules/tools-engine/SPEC.md): [`FakeEngine`] trzyma świat
//! w pamięci i odtwarza zachowanie serwera NextGO.Gray ważne dla Alfy — rewizje, idempotentne
//! transakcje, konflikt rewizji, historię cofania (64), kroki, zaznaczenie i błędy z `data.code`.
//! Deterministyczna; zrzuty i podglądy to obraz PNG 1×1 w katalogu podanym przy tworzeniu.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod world;

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use serde_json::Value;
use tools_engine_contract::{
    Capture, DocEntry, EngineError, EngineMethod, EnginePort, EngineState, Installation,
    PreviewOptions, StartOptions,
};

pub use world::World;

/// Najmniejszy poprawny PNG (1×1, przezroczysty).
pub const PIXEL_PNG: [u8; 68] = [
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0B, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x60, 0x00, 0x02, 0x00,
    0x00, 0x05, 0x00, 0x01, 0x7A, 0x5E, 0xAB, 0x3F, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44,
    0xAE, 0x42, 0x60, 0x82,
];

struct State {
    running: Option<(EngineState, World)>,
    calls: Vec<EngineMethod>,
}

/// Atrapa silnika.
pub struct FakeEngine {
    dir: PathBuf,
    installed: bool,
    state: Mutex<State>,
}

impl FakeEngine {
    /// Atrapa zainstalowanego silnika; `dir` — katalog na zrzuty i podglądy.
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            installed: true,
            state: Mutex::new(State {
                running: None,
                calls: Vec::new(),
            }),
        }
    }

    /// Atrapa bez wskazanego katalogu silnika (`NotConfigured`).
    pub fn unconfigured(dir: PathBuf) -> Self {
        Self {
            installed: false,
            ..Self::new(dir)
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Wywołane metody (do asercji w testach).
    pub fn calls(&self) -> Vec<EngineMethod> {
        self.lock().calls.clone()
    }

    fn write_png(&self, name: &str) -> Result<PathBuf, EngineError> {
        std::fs::create_dir_all(&self.dir).map_err(|e| EngineError::Io(e.to_string()))?;
        let path = self.dir.join(name);
        std::fs::write(&path, PIXEL_PNG).map_err(|e| EngineError::Io(e.to_string()))?;
        Ok(path)
    }
}

#[async_trait]
impl EnginePort for FakeEngine {
    fn installation(&self) -> Installation {
        if !self.installed {
            return Installation::default();
        }
        Installation {
            root: Some(self.dir.join("silnik")),
            server: Some(self.dir.join("silnik/target/release/engine-server.exe")),
            bridge: None,
            version: Some("0.1.0".into()),
            verification: Some("passed".into()),
            verified_utc: Some("2026-10-26T18:02:36Z".into()),
        }
    }

    fn state(&self) -> EngineState {
        self.lock()
            .running
            .as_ref()
            .map(|(s, _)| s.clone())
            .unwrap_or_default()
    }

    async fn start(&self, options: StartOptions) -> Result<EngineState, EngineError> {
        if !self.installed {
            return Err(EngineError::NotConfigured);
        }
        let mut st = self.lock();
        if let Some((s, _)) = &st.running {
            return Err(EngineError::AlreadyRunning(
                s.url.clone().unwrap_or_default(),
            ));
        }
        let state = EngineState {
            running: true,
            url: Some("ws://127.0.0.1:50000".into()),
            mode: Some(
                if options.window {
                    "gpu_preview"
                } else {
                    "headless"
                }
                .into(),
            ),
            paused: options.paused || options.window,
            window: options.window,
            pid: Some(4242),
            scene: Some(format!("{:?}", options.scene).to_lowercase()),
            exit_code: None,
        };
        st.running = Some((state.clone(), World::new(options.scene)));
        Ok(state)
    }

    async fn stop(&self) -> Result<bool, EngineError> {
        Ok(self.lock().running.take().is_some())
    }

    async fn call(&self, method: EngineMethod, params: Value) -> Result<Value, EngineError> {
        let mut st = self.lock();
        st.calls.push(method);
        let (_, world) = st.running.as_mut().ok_or(EngineError::NotRunning)?;
        world.call(method, params)
    }

    async fn capture(&self) -> Result<Option<Capture>, EngineError> {
        let (window, revision) = {
            let st = self.lock();
            let (s, w) = st.running.as_ref().ok_or(EngineError::NotRunning)?;
            (s.window, w.revision())
        };
        if !window {
            return Ok(None);
        }
        Ok(Some(Capture {
            png: self.write_png(&format!("revision-{revision}.png"))?,
            revision: Some(revision),
            telemetry: serde_json::json!({"revision": revision, "triangles": 12}),
        }))
    }

    async fn preview(&self, options: PreviewOptions) -> Result<Capture, EngineError> {
        if !self.installed {
            return Err(EngineError::NotConfigured);
        }
        Ok(Capture {
            png: self.write_png("preview.png")?,
            revision: Some(0),
            telemetry: serde_json::json!({"width": options.width, "height": options.height,
                                          "simulation_frame": options.frames}),
        })
    }

    async fn docs(&self) -> Result<Vec<DocEntry>, EngineError> {
        if !self.installed {
            return Err(EngineError::NotConfigured);
        }
        Ok(vec![DocEntry {
            name: "docs/API_ETAP_4.md".into(),
            bytes: 64,
            title: "API etapu 4".into(),
        }])
    }

    async fn read_doc(&self, name: &str) -> Result<String, EngineError> {
        if name == "docs/API_ETAP_4.md" {
            Ok("# API etapu 4\n`entity.spawn` tworzy encję.".into())
        } else {
            Err(EngineError::Io(format!("nie ma pliku „{name}”")))
        }
    }

    fn kill_all(&self) -> usize {
        usize::from(self.lock().running.take().is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_passes_the_port_scenario() {
        let d = tempfile::tempdir().unwrap();
        let fake = FakeEngine::new(d.path().to_path_buf());
        tools_engine_contract::contract_tests::port_scenario(&fake).await;
        assert!(fake.calls().contains(&EngineMethod::RollbackLastChange));
    }

    #[tokio::test]
    async fn unconfigured_fake_refuses_to_start() {
        let d = tempfile::tempdir().unwrap();
        let fake = FakeEngine::unconfigured(d.path().to_path_buf());
        assert_eq!(
            fake.start(StartOptions::default()).await,
            Err(EngineError::NotConfigured)
        );
        assert_eq!(fake.installation(), Installation::default());
        assert_eq!(fake.kill_all(), 0);
    }
}
