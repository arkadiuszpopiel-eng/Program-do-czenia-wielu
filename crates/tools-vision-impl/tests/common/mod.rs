//! Wspólne środowisko testów `tools-vision`: wirtualny pulpit (Notatnik z polem hasła, okno Alfy),
//! pliki (obraz, bomba dekompresyjna, nie-obraz, film, BMP, plik z deny-listy), atrapy OCR, opisu,
//! prywatności i Brokera z prawdziwym silnikiem.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use compliance_contract::{DenyLists, PathEnv};
use core_bus_fake::FakeBus;
use lib_media::{PortFiles, samples};
use platform_contract::{ScreenRect, WindowId};
use platform_fake::{FakeDesktop, FakeElement, FakeFs, FakeWindow};
use safety_broker_contract::{Holder, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use tools_common_contract::{Tool, ToolCtx, Toolset};
use tools_vision_contract::{VisionPrivacy, VisionToolsConfig};
use tools_vision_fake::{FakeDescriber, FakeOcr, FakePrivacy, line};
use tools_vision_impl::{VisionTools, VisionToolsDeps};
use watchdog_contract::ManualClock;

const HOME: &str = "/Users/ala";

pub struct Env {
    pub desktop: Arc<FakeDesktop>,
    pub broker: Arc<FakeBroker>,
    pub bus: Arc<FakeBus>,
    pub ocr: Arc<FakeOcr>,
    pub describer: Arc<FakeDescriber>,
    pub privacy: Arc<FakePrivacy>,
    pub notepad: WindowId,
    pub alfa: WindowId,
    pub tools: VisionTools,
}

pub fn env_with(config: VisionToolsConfig, describer: FakeDescriber) -> Env {
    let desktop = Arc::new(FakeDesktop::new());
    let notepad = desktop.add_window(
        FakeWindow::new(
            "Notatnik",
            "notepad.exe",
            ScreenRect::from_xywh(0, 0, 900, 700),
        ),
        true,
    );
    desktop.add_element(
        notepad,
        FakeElement::new("Hasło", "edit", ScreenRect::from_xywh(50, 50, 200, 25)).password(),
    );
    let alfa = desktop.add_window(
        FakeWindow::new(
            "Alfa",
            "alfa-desktop.exe",
            ScreenRect::from_xywh(1000, 0, 900, 700),
        ),
        false,
    );
    let fs = Arc::new(FakeFs::with_files([
        (
            PathBuf::from("/Users/ala/Obrazy/zrzut.png"),
            samples::png(800, 600, 0),
        ),
        (
            PathBuf::from("/Users/ala/Obrazy/bomba.png"),
            samples::png(60_000, 60_000, 0),
        ),
        (
            PathBuf::from("/Users/ala/Obrazy/notatka.txt"),
            b"to nie obraz".to_vec(),
        ),
        (
            PathBuf::from("/Users/ala/Obrazy/film.mp4"),
            samples::mp4(64, 64, 1000, false),
        ),
        (
            PathBuf::from("/Users/ala/Obrazy/stary.bmp"),
            samples::bmp(64, 64),
        ),
        (
            PathBuf::from("/Users/ala/.ssh/klucz.png"),
            samples::png(10, 10, 0),
        ),
    ]));
    let env = PathEnv::windows_profile(HOME);
    let policy = KernelPolicy::baseline(HOME, "/ProgramData/AlfaBroker").unwrap();
    let broker = Arc::new(
        FakeBroker::with(policy, env.clone(), Arc::new(ManualClock::new(1_000_000))).unwrap(),
    );
    for t in ["tools-vision.ocr", "tools-vision.describe"] {
        broker.script(t, ScriptedDecision::Allow);
    }
    let ocr = Arc::new(FakeOcr::new(vec![
        line("Plik Edycja Widok", 10.0, 5.0, 300.0, 20.0),
        line(
            "hasło: sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789",
            10.0,
            40.0,
            400.0,
            20.0,
        ),
    ]));
    let describer = Arc::new(describer);
    let privacy = Arc::new(FakePrivacy::default());
    privacy.set("s1", VisionPrivacy::Normal);
    let bus = Arc::new(FakeBus::default());
    let tools = VisionTools::new(VisionToolsDeps {
        desktop: desktop.clone(),
        capture: desktop.clone(),
        ocr: ocr.clone(),
        describer: describer.clone(),
        privacy: privacy.clone(),
        files: Arc::new(PortFiles::new(fs)),
        broker: broker.clone(),
        env,
        deny: DenyLists::baseline(),
        config,
        bus: Some(bus.clone()),
    });
    Env {
        desktop,
        broker,
        bus,
        ocr,
        describer,
        privacy,
        notepad,
        alfa,
        tools,
    }
}

/// Atrapa koduje PNG bez kompresji — limity podniesione dla zrzutów całego monitora.
pub fn config() -> VisionToolsConfig {
    VisionToolsConfig {
        max_file_bytes: 32 * 1024 * 1024,
        describe_max_bytes: 32 * 1024 * 1024,
        ..VisionToolsConfig::default()
    }
}

pub fn env() -> Env {
    env_with(config(), FakeDescriber::new(true, true))
}

impl Env {
    pub fn tool(&self, name: &str) -> Arc<dyn Tool> {
        self.tools
            .tools()
            .into_iter()
            .find(|t| t.manifest().name == name)
            .unwrap()
    }

    pub fn events(&self) -> String {
        self.bus
            .recorded()
            .iter()
            .map(|ev| serde_json::to_string(&ev.payload).unwrap())
            .collect()
    }
}

pub fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta")).with_workdir("/Users/ala/Obrazy");
    c.approval_timeout = Duration::from_millis(300);
    c
}
