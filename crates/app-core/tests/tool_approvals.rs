//! Diagnostyka narzędzi, które czekają na zgodę właściciela (docs/plan/45 §6.5): ten sam rdzeń
//! z atrapami co `tool_diag.rs`, ale z PRAWDZIWYM Brokerem poza procesem i jego oknem zgód —
//! `target/debug/alfa-broker.exe --console` z własną konfiguracją (osobny potok, dane Brokera
//! i profil w katalogu tymczasowym, klient = ten program testowy). Karty zatwierdza właściciel
//! ręcznie; test czeka na każdą do 3 min. Narzędzia działają na atrapach (przeglądarka, pliki) —
//! zgoda niczego nie zmienia poza katalogiem testu. Uruchamiany tylko na żądanie:
//!
//! ```text
//! cargo build -p app-safety --bins
//! cargo test -p app-core --test tool_approvals -- --ignored --nocapture
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod tool_rig;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use app_broker::link::{BrokerLink, LinkConfig, ServerCheck};
use app_broker::{KernelStatus, RemoteKernel};
use app_core::dto::{AutonomyLevel, SessionTemplate};
use platform_contract::ProcessIdentityPort;
use platform_windows_kernel_impl::WinKernel;
use safety_broker_contract::Holder;
use serde_json::{Value, json};
use tool_rig::report;
use tools_common_contract::{Tool, ToolCtx};

/// Ile czekać na decyzję właściciela przy jednej karcie.
const OWNER: Duration = Duration::from_secs(180);

/// Katalog binarek Jądra zbudowanych przez `cargo build -p app-safety --bins`.
fn bin_dir() -> PathBuf {
    plain(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug"))
}

/// Ścieżka bez `..` i bez przedrostka `\\?\` (tak, jak Windows podaje obraz procesu).
fn plain(path: &Path) -> PathBuf {
    let full = std::fs::canonicalize(path).unwrap();
    let text = full.to_string_lossy();
    PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned())
}

/// Broker poza procesem z oknem zgód; zakończenie procesu przy końcu testu (`Drop`).
struct BrokerProcess(Child);

impl Drop for BrokerProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Uruchamia `alfa-broker --console` z konfiguracją w `root` i łączy się z nim.
fn start_broker(root: &Path) -> (BrokerProcess, RemoteKernel) {
    let bin = bin_dir();
    let image = bin.join("alfa-broker.exe");
    assert!(
        image.is_file(),
        "brak {} — najpierw `cargo build -p app-safety --bins`",
        image.display()
    );
    let user = WinKernel.current_user().unwrap();
    let pipe = format!("alfa-broker-diag-{}", std::process::id());
    let mut config =
        app_safety::broker::dev_config(&bin, user, &root.to_string_lossy(), root.join("broker"));
    config.pipe_name = pipe.clone();
    if let Some(core) = config.bindings.core.as_mut() {
        core.requirement.images = vec![plain(&std::env::current_exe().unwrap())];
    }
    config.bindings.watchdog = None;
    let path = root.join("broker.json");
    std::fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
    let child = Command::new(&image)
        .args(["--console", "--config"])
        .arg(&path)
        .arg("--lifeline")
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id();
    let process = BrokerProcess(child);
    let status = Arc::new(KernelStatus::new(app_api::dto::BrokerMode::Portable, true));
    let kernel = Arc::new(WinKernel);
    let link = BrokerLink::new(kernel.clone(), kernel, LinkConfig::new(pipe), status);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match link.connect(&ServerCheck::Pid(pid)) {
            Ok(()) => break,
            Err(e) if Instant::now() > deadline => panic!("Sędzia Cieni nie odpowiada: {e}"),
            Err(_) => std::thread::sleep(Duration::from_millis(250)),
        }
    }
    (process, RemoteKernel::new(link))
}

/// Próba: narzędzie, argumenty, co zobaczy właściciel, oczekiwana klasa wyniku i poziom
/// autonomii sesji (`None` = domyślny Brokera, L3).
struct Probe {
    name: &'static str,
    args: Value,
    card: &'static str,
    expect: &'static str,
    level: Option<AutonomyLevel>,
}

fn probe(name: &'static str, args: Value, card: &'static str) -> Probe {
    Probe {
        name,
        args,
        card,
        expect: "ok",
        level: None,
    }
}

/// Narzędzia do sprawdzenia. Trwałe usunięcie dwa razy: na L3 Broker nie pyta, więc narzędzie
/// nic nie usuwa i zwraca intencję potwierdzenia (bez karty); na L2 Broker pokazuje kartę.
fn probes() -> Vec<Probe> {
    let page = tool_rig::fakes::PAGE;
    let delete = json!({ "path": "do-usuniecia-trwale.txt" });
    vec![
        probe(
            "browser_open",
            json!({ "url": page }),
            "otwarcie strony example.com (atrapa)",
        ),
        probe(
            "browser_click",
            json!({ "node": 1 }),
            "kliknięcie przycisku „Wyślij”",
        ),
        probe(
            "browser_type",
            json!({ "node": 2, "text": "diagnostyka" }),
            "wpisanie „diagnostyka”",
        ),
        probe(
            "browser_select",
            json!({ "node": 3, "value": "Polska" }),
            "wybór „Polska” z listy",
        ),
        probe(
            "browser_key",
            json!({ "key": "enter" }),
            "naciśnięcie Enter na stronie",
        ),
        probe(
            "browser_click",
            json!({ "node": 4 }),
            "kliknięcie odnośnika „Więcej” (druga strona w historii)",
        ),
        probe(
            "browser_nav",
            json!({ "action": "back" }),
            "cofnięcie w historii karty",
        ),
        Probe {
            expect: "potwierdzenie",
            ..probe(
                "fs_delete_permanent",
                delete.clone(),
                "BEZ karty (poziom L3) — narzędzie nic nie usuwa",
            )
        },
        Probe {
            level: Some(AutonomyLevel::L2),
            ..probe(
                "fs_delete_permanent",
                delete,
                "trwałe usunięcie pliku testowego na L2 (atrapa systemu plików)",
            )
        },
    ]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "wymaga ręcznej zgody właściciela w oknie Sędziego Cieni"]
async fn waiting_tools_run_after_owner_approval_in_broker_window() {
    let broker = std::sync::Mutex::new(None);
    let kernel = |root: &Path| {
        let (process, kernel) = start_broker(root);
        *broker.lock().unwrap() = Some(process);
        kernel
    };
    let rig = tool_rig::fakes::rig_with(Some(&kernel)).await;
    let tools = rig.core.agent_tools();
    let browser = rig.browser_session.clone();
    eprintln!("\nOkno Sędziego Cieni: zatwierdzaj karty po kolei (każda czeka do 3 min):");
    for p in probes() {
        eprintln!("  - {}: {}", p.name, p.card);
    }
    let exists = || {
        rig.fs
            .snapshot()
            .keys()
            .any(|p| p.ends_with("do-usuniecia-trwale.txt"))
    };
    let mut lines = Vec::new();
    let mut problems = Vec::new();
    for p in probes() {
        let tool = tools
            .iter()
            .find(|t| t.manifest().name == p.name)
            .unwrap()
            .clone();
        let session = if p.name.starts_with("browser_") {
            browser.clone()
        } else {
            rig.core
                .sessions_create(SessionTemplate::Empty)
                .await
                .unwrap()
                .id
        };
        let level = p.level.unwrap_or_default();
        if let Some(lower) = p.level {
            // Obniżenie działa od razu, bez karty.
            rig.core
                .permissions_request_level(lower, Some(session.clone()))
                .await
                .unwrap();
        }
        let mut ctx = ToolCtx::new(Holder::agent(session.as_str(), "delta"))
            .with_workdir(&rig.workdir.to_string_lossy());
        ctx.approval_timeout = OWNER;
        eprintln!("→ {} ({level:?})…", p.name);
        let started = Instant::now();
        let out = call(tool, p.args.clone(), ctx).await;
        let class = report::class(&out.status);
        let secs = started.elapsed().as_secs();
        let text = first_line(&out.text);
        eprintln!("  {class} po {secs} s: {text}");
        if class != p.expect {
            problems.push(format!(
                "{} ({level:?}): {class}, oczekiwano {} — {text}",
                p.name, p.expect
            ));
        }
        if p.name == "fs_delete_permanent" && (class == "ok") == exists() {
            problems.push(format!(
                "fs_delete_permanent ({level:?}): stan pliku niezgodny z wynikiem"
            ));
        }
        lines.push(format!(
            "| `{}` | {level:?} | {class} | {} | {secs} | `{}` | {text} |",
            p.name, p.expect, p.args
        ));
    }
    full_automation_shows_no_window(&rig, &tools, &browser, &mut lines, &mut problems).await;
    write_report(&lines, &problems);
    drop(rig);
    drop(broker.lock().unwrap().take());
    assert!(
        problems.is_empty(),
        "narzędzia po zgodzie:\n{}",
        problems.join("\n")
    );
}

/// Część 2 (decyzja właściciela 31.10): sesja przeglądarki podniesiona do „Pełnej automatyki”
/// jedną kartą zmiany trybu, potem te same działania na stronie po obcej treści — każde ma się
/// wykonać od razu, bez żadnego okna (limit zgody 15 s: karta skończyłaby się odmową).
async fn full_automation_shows_no_window(
    rig: &tool_rig::fakes::Rig,
    tools: &[Arc<dyn Tool>],
    browser: &str,
    lines: &mut Vec<String>,
    problems: &mut Vec<String>,
) {
    eprintln!("→ podniesienie sesji przeglądarki do L4 — kliknij „Włącz: Pełna automatyka”…");
    rig.core
        .permissions_request_level(AutonomyLevel::L4, Some(browser.to_owned()))
        .await
        .unwrap();
    let deadline = Instant::now() + OWNER;
    loop {
        let state = rig
            .core
            .permissions_get(Some(browser.to_owned()))
            .await
            .unwrap();
        if state.session == Some(AutonomyLevel::L4) {
            break;
        }
        if Instant::now() > deadline {
            problems.push("L4: zmiana trybu nie zatwierdzona w 3 min".into());
            return;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    eprintln!("  L4 włączone — teraz NIE powinno pokazać się żadne okno:");
    let steps = [
        ("browser_click", json!({ "node": 1 })),
        (
            "browser_type",
            json!({ "node": 2, "text": "pełna automatyka" }),
        ),
        ("browser_select", json!({ "node": 3, "value": "Polska" })),
        ("browser_key", json!({ "key": "enter" })),
    ];
    for (name, args) in steps {
        let tool = tools
            .iter()
            .find(|t| t.manifest().name == name)
            .unwrap()
            .clone();
        let mut ctx = ToolCtx::new(Holder::agent(browser, "delta"))
            .with_workdir(&rig.workdir.to_string_lossy());
        ctx.approval_timeout = Duration::from_secs(15);
        let started = Instant::now();
        let out = call(tool, args.clone(), ctx).await;
        let (class, ms) = (report::class(&out.status), started.elapsed().as_millis());
        let text = first_line(&out.text);
        eprintln!("  {name} (L4): {class} po {ms} ms");
        if class != "ok" || ms > 5_000 {
            problems.push(format!("{name} (L4): {class} po {ms} ms — {text}"));
        }
        lines.push(format!(
            "| `{name}` | L4 | {class} | ok, bez okna | 0 | `{args}` | {text} |"
        ));
    }
}

async fn call(
    tool: Arc<dyn Tool>,
    args: Value,
    ctx: ToolCtx,
) -> tools_common_contract::ToolOutcome {
    tokio::spawn(async move { tool.call(args, &ctx).await })
        .await
        .unwrap()
}

fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(160)
        .collect::<String>()
        .replace('|', "\\|")
}

fn write_report(lines: &[String], problems: &[String]) {
    let dir = report::out_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let mut md = String::from(
        "# Narzędzia po ręcznej zgodzie właściciela\n\nBroker poza procesem z oknem zgód; narzędzia na atrapach.\n\n",
    );
    for p in problems {
        md.push_str(&format!("- **{p}**\n"));
    }
    md.push_str("\n| Narzędzie | Poziom | Wynik | Oczekiwany | s | Argumenty | Tekst |\n|---|---|---|---|---|---|---|\n");
    md.push_str(&lines.join("\n"));
    md.push('\n');
    std::fs::write(dir.join("zgody.md"), md).unwrap();
}
