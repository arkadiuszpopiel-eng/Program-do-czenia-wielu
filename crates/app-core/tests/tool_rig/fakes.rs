//! Rdzeń diagnostyki narzędzi: KAŻDY port, przez który narzędzie działa na maszynie, jest atrapą
//! (pliki, polecenia, schowek, pulpit, aplikacje, system, sieć, silnik, diagnostyka komputera).
//! Poziom autonomii L4 zapisany w profilu tymczasowym (ten sam plik co `ALFA_TEST_AUTONOMY`),
//! więc Broker przepuszcza działania jak na instancji z pełnym dostępem — twarde blokady Jądra
//! działają dalej i trafiają do raportu jako odmowy.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use app_agents::{AppsDeps, PortOverrides, SysNetDeps};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use platform_apps_contract::{BrowserKind, BrowserSpec, ServiceState};
use platform_apps_fake::{
    FakeBrowser, FakeDownloads, FakeNode, FakeOffice, FakePage, FakeSys, fake_process,
};
use platform_contract::{ExecSpec, ScreenRect, UiaPattern};
use platform_fake::{
    FakeClipboard, FakeDesktop, FakeElement, FakeExec, FakeFs, FakePty, FakeSignals, FakeWindow,
};
use providers_fake::{FAKE_MODEL, FakeProvider, Script};
use tools_system_contract::{SoftwareItem, SoftwareKind, SoftwareProbe};

use super::files::{FILES, documents};

/// Cele w atrapach dla przepisów (okno, elementy UI, proces, usługa).
#[derive(Debug, Clone)]
pub struct Targets {
    pub window: u64,
    pub button: String,
    pub edit: String,
    pub pid: u32,
    pub service: &'static str,
}

/// Rdzeń z atrapami i katalog roboczy narzędzi.
pub struct Rig {
    pub core: AppCore,
    pub workdir: PathBuf,
    pub fs: Arc<FakeFs>,
    pub exec: Arc<FakeExec>,
    pub ocr: Arc<tools_vision_fake::FakeOcr>,
    pub targets: Targets,
    /// Sesja narzędzi przeglądarki (przeglądarka należy do sesji `browser_open`).
    pub browser_session: String,
    _dir: tempfile::TempDir,
}

/// Wykrywanie programów bez uruchamiania czegokolwiek (jeden program „zainstalowany”).
struct OneProgram;

impl SoftwareProbe for OneProgram {
    fn scan(&self) -> Vec<SoftwareItem> {
        vec![SoftwareItem {
            id: "node".into(),
            name: "Node.js".into(),
            kind: SoftwareKind::Runtime,
            version: Some("22.0.0".into()),
            command: "node".into(),
        }]
    }
}

/// Porty aplikacji, systemu i sieci na atrapach (`browser_spec` w katalogu danych testu).
fn apps(local: &Path, sys: Arc<FakeSys>, desktop: Arc<FakeDesktop>) -> AppsDeps {
    AppsDeps {
        office: Arc::new(FakeOffice::new()),
        browser: Arc::new(browser()),
        browser_spec: BrowserSpec {
            kind: BrowserKind::Edge,
            executable: None,
            alfa_root: local.to_path_buf(),
            profile_dir: local.join("browser").join("profile"),
            quarantine_dir: local.join("browser").join("quarantine"),
            headless: true,
        },
        plugins_dir: None,
        sysnet: Some(SysNetDeps {
            sys,
            power: Some(Arc::new(FakeSignals::default())),
            desktop: Some(desktop),
            hardware: None,
            http: Arc::new(http()),
            search: None,
            downloads: Arc::new(FakeDownloads::default()),
            quarantine_root: Some(local.join("kwarantanna")),
            software: Some(Arc::new(OneProgram)),
        }),
    }
}

/// Strona przeglądarki atrapy: przycisk (węzeł 1), pole (2), lista (3), odnośnik (4).
pub const PAGE: &str = "https://example.com/";

fn browser() -> FakeBrowser {
    let browser = FakeBrowser::new();
    let mut select = FakeNode::new("combobox", "Kraj");
    select.options = vec!["Polska".into(), "Niemcy".into()];
    let mut link = FakeNode::new("link", "Więcej");
    link.href = Some(format!("{PAGE}wiecej"));
    browser.add_page(
        PAGE,
        FakePage {
            title: "Przykład".into(),
            text: "Strona przykładowa diagnostyki.".into(),
            nodes: vec![
                FakeNode::new("button", "Wyślij"),
                FakeNode::new("textbox", "Szukaj"),
                select,
                link,
            ],
            ..FakePage::default()
        },
    );
    browser.add_page(&format!("{PAGE}wiecej"), FakePage::default());
    browser
}

/// Sieć atrapy: strona HTML i plik tekstowy do pobrania.
fn http() -> tools_net_fake::FakeHttp {
    let http = tools_net_fake::FakeHttp::default();
    http.page(
        PAGE,
        "text/html",
        b"<html><body><p>Strona diagnostyki.</p></body></html>",
    );
    http.page(
        &format!("{PAGE}plik.txt"),
        "text/plain",
        b"plik do pobrania
",
    );
    http
}

/// Pulpit atrapy: okno Notatnika z przyciskiem i polem tekstowym.
fn desktop() -> (Arc<FakeDesktop>, u64, String, String) {
    let desktop = Arc::new(FakeDesktop::new());
    let rect = |left, top, right, bottom| ScreenRect {
        left,
        top,
        right,
        bottom,
    };
    let window = desktop.add_window(
        FakeWindow::new(
            "Notatki — Notatnik",
            "notepad.exe",
            rect(100, 100, 900, 700),
        ),
        true,
    );
    let button = FakeElement::new("Zapisz", "button", rect(120, 120, 200, 150))
        .patterns(&[UiaPattern::Invoke]);
    let edit = FakeElement::new("Treść", "edit", rect(120, 200, 880, 680))
        .patterns(&[UiaPattern::Value, UiaPattern::Text])
        .value("Ala ma kota.")
        .text(
            "Ala ma kota.
Druga linia.",
        );
    let button = desktop.add_element(window, button).unwrap().to_string();
    let edit = desktop.add_element(window, edit).unwrap().to_string();
    (desktop, window.0, button, edit)
}

/// Buduje rdzeń: pliki katalogu roboczego w atrapie FS i na dysku, poziom L4 w profilu testu.
pub async fn rig() -> Rig {
    rig_with(None).await
}

/// Broker poza procesem dla rdzenia diagnostyki (katalog główny testu = profil właściciela).
pub type KernelFor<'a> = &'a dyn Fn(&Path) -> app_broker::RemoteKernel;

/// Jak [`rig`]; `kernel` — Broker poza procesem z oknem zgód zamiast Brokera w procesie.
pub async fn rig_with(kernel: Option<KernelFor<'_>>) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::under(dir.path());
    let workdir = paths.user_root.join("Praca");
    let static_files = FILES.iter().map(|(name, body)| (*name, body.to_vec()));
    let seeded = static_files.chain(documents()).map(|(name, body)| {
        let path = workdir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &body).unwrap();
        (path, body)
    });
    let fs = Arc::new(FakeFs::with_files(seeded.collect::<Vec<_>>()));
    let broker_dir = app_broker::inproc::dev_dir(&paths.local);
    std::fs::create_dir_all(&broker_dir).unwrap();
    let standing = broker_dir.join(app_broker::inproc::STANDING_FILE);
    std::fs::write(standing, r#"{"level":"L4"}"#).unwrap();
    // ffmpeg „zainstalowany” (pusty plik): konwersja idzie do atrapy poleceń, nie do programu.
    let ffmpeg = app_agents::ffmpeg_path(&paths);
    std::fs::create_dir_all(ffmpeg.parent().unwrap()).unwrap();
    std::fs::write(&ffmpeg, b"").unwrap();

    let exec = Arc::new(FakeExec::new());
    exec.set_default(platform_fake::FakeRun::ok("wynik atrapy\n"));
    // ffmpeg atrapy zapisuje plik wynikowy (ostatni argument `file:<ścieżka>`), jak konwersja.
    let command = ffmpeg.to_string_lossy().into_owned();
    let is_ffmpeg = move |spec: &ExecSpec| spec.process.cmd == command;
    let write_output = |spec: &ExecSpec| {
        let out = spec
            .process
            .args
            .last()
            .map(|a| a.trim_start_matches("file:"));
        if let Some(out) = out {
            let _ = std::fs::write(out, b"ID3atrapa");
        }
    };
    exec.on(
        is_ffmpeg,
        platform_fake::FakeRun::ok("").with_effect(write_output),
    );
    let sys = Arc::new(FakeSys::default());
    sys.add_process(fake_process(4321, 1200, "kalkulator.exe"));
    sys.add_service("DiagUsluga", ServiceState::Stopped);
    let (desktop, window, button, edit) = desktop();
    let mut opts = crate::common::options(None, Arc::new(HeadlessShell::default()));
    opts.fs = Some(fs.clone());
    opts.exec = Some(exec.clone());
    opts.clipboard = Some(Arc::new(FakeClipboard::new()));
    opts.gui = Some(app_gui::GuiPorts::from_one(desktop.clone(), true));
    opts.pty = Some(Arc::new(FakePty::default()));
    opts.diag_probe = Some(Arc::new(pc_diag_fake::FakeProbe::default()));
    opts.approval_timeout = Some(Duration::from_millis(300));
    opts.kernel = kernel.map(|k| k(dir.path()));
    // Model z obsługą obrazów (lokalny) dla `vision_describe` — odpowiedź ze skryptu.
    let script = Script::text(FAKE_MODEL, &["Biały piksel na białym tle."]);
    let vision = FakeProvider::new("diag-wizja").with_default_script(script);
    opts.providers = vec![(Arc::new(vision), router_contract::RouteKind::Local)];
    let ocr = Arc::new(tools_vision_fake::FakeOcr::default());
    opts.tool_ports = PortOverrides {
        apps: Some(apps(&paths.local, sys, desktop)),
        engine: Some(Arc::new(tools_engine_fake::FakeEngine::new(
            paths.local.join("engine"),
        ))),
        ocr: Some(ocr.clone()),
    };
    let core = AppCore::build(paths, opts).await.unwrap();
    let browser_session = core
        .sessions_create(app_core::dto::SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let targets = Targets {
        window,
        button,
        edit,
        pid: 4321,
        service: "DiagUsluga",
    };
    Rig {
        core,
        workdir,
        fs,
        exec,
        ocr,
        targets,
        browser_session,
        _dir: dir,
    }
}
