//! F8 przez komendy `AppCore` (atrapy): Delta na wirtualnym pulpicie — sterowanie bez zgody
//! właściciela (okno Brokera) odmówione po czasie, wobec okien Alfy — blokada Jądra (0 skutków);
//! panel „Ekran" i `GuiActivity` bez wpisywanej treści; „Zatrzymaj sterowanie" / „Oddaj". Terminal: wejście tylko komendą (gest), treść nigdy w zdarzeniach
//! (szpieg). Umiejętność: propozycja → przegląd → zatwierdzenie hashem → uruchomienie = zadanie.
//! Kreator: zapis dopiero po teście na sucho. Diagnosta: błąd 401 dostawcy → incydent →
//! naprawa → „Cofnij".

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use accounts_hub_contract::CliProbe;
use app_core::dto::{
    AgentDraft, AlfaEvent, GuiActionStatus, SkillStateView, TerminalFrame, TerminalProfileId,
};
use app_core::{AppCore, AppPaths, FrameSink};
use app_gui::GuiPorts;
use common::agents::*;
use common::*;
use platform_contract::{ScreenRect, TargetGuard, WindowId};
use platform_fake::{FakeDesktop, FakePty, FakeWindow};
use providers_contract::{ProviderError, ProviderErrorKind};
use providers_fake::{FAKE_MODEL, Script};
use serde_json::json;

const SECRET: &str = "Lista zakupow mleko 4417";
/// PowerShell 7: ścieżka bezwzględna dla systemu (`PtySpec::validate`; `/usr/…` na Windows — nie).
#[cfg(windows)]
const PWSH: &str = r"C:\Program Files\PowerShell\7\pwsh.exe";
#[cfg(not(windows))]
const PWSH: &str = "/usr/bin/pwsh";

struct Probe;

impl CliProbe for Probe {
    fn locate(&self, program: &str) -> Option<PathBuf> {
        (program == "pwsh").then(|| PathBuf::from(PWSH))
    }
    fn version_output(&self, _path: &Path) -> Option<String> {
        None
    }
}

struct Desk {
    desktop: Arc<FakeDesktop>,
    notepad: WindowId,
    alfa: Vec<WindowId>,
}

fn desk() -> Desk {
    let guard = TargetGuard::baseline()
        .with_pids([4_242])
        .with_image_dirs([r"C:\Users\ala\AppData\Local\Alfa"]);
    let desktop = Arc::new(FakeDesktop::with_guard(guard));
    let rect = |x| ScreenRect::from_xywh(x, 0, 500, 400);
    let notepad = desktop.add_window(
        FakeWindow::new("Notatnik", r"C:\Windows\notepad.exe", rect(0)),
        true,
    );
    let alfa = vec![
        desktop.add_window(
            FakeWindow::new("Alfa", r"C:\Program Files\Alfa\alfa-desktop.exe", rect(520)),
            false,
        ),
        desktop.add_window(
            FakeWindow::new(
                "WebView",
                r"C:\Users\ala\AppData\Local\Alfa\versions\1\msedgewebview2.exe",
                rect(1040),
            ),
            false,
        ),
    ];
    Desk {
        desktop,
        notepad,
        alfa,
    }
}

/// Rdzeń z Deltą, wirtualnym pulpitem i atrapą pseudokonsoli.
async fn world(d: &Desk, pty: Arc<FakePty>) -> Agents {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(app_core::ports::HeadlessShell::default());
    let mut opts = options(Some(provider.clone()), shell.clone());
    opts.exec = Some(Arc::new(platform_fake::FakeExec::new()));
    opts.approval_timeout = Some(Duration::from_secs(1));
    opts.gui = Some(GuiPorts::from_one(d.desktop.clone(), true));
    opts.pty = Some(pty);
    opts.cli_probe = Some(Arc::new(Probe));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    agents_on(core, provider, shell, dir).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn delta_on_virtual_desktop_never_touches_alfa_windows() {
    let d = desk();
    let mut a = world(&d, Arc::new(FakePty::default())).await;
    let call = |id: &str, tool: &str, args: serde_json::Value| {
        Script::tool_call(FAKE_MODEL, id, tool, &args)
    };
    a.h.provider.push(call("c1", "window_list", json!({})));
    a.h.provider
        .push(call("c2", "window_focus", json!({ "window": d.alfa[0].0 })));
    a.h.provider.push(call(
        "c3",
        "input_type_text",
        json!({ "window": d.alfa[1].0, "text": "x" }),
    ));
    // Wpisywanie w cudzym oknie wymaga zgody właściciela (okno Brokera) — bez decyzji: odmowa.
    a.h.provider.push(call(
        "c4",
        "input_type_text",
        json!({ "window": d.notepad.0, "text": SECRET }),
    ));
    a.h.provider.push(Script::text(FAKE_MODEL, &["Gotowe."]));
    let (_turn, events) = run_turn(&mut a, "Delta, wpisz listę zakupów w Notatniku.").await;
    let run = last_run(&a).await;
    assert_eq!(
        d.desktop.typed_text(d.notepad),
        "",
        "bez zgody nic nie wpisano: {run:#?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AlfaEvent::ApprovalPending { .. })),
        "karta „czeka na zatwierdzenie” dla wpisywania"
    );
    let on_alfa = d
        .desktop
        .records()
        .iter()
        .filter(|r| d.alfa.contains(&r.window))
        .count();
    assert_eq!(on_alfa, 0, "0 akcji w oknach Alfy");
    let status = a.h.core.gui_status().await.unwrap();
    assert!(status.available);
    // Sterowanie GUI bez zgody właściciela (okno Brokera / „zawsze zezwalaj”) — odmowa po czasie;
    // wobec okien Alfy — blokada Jądra niezależnie od zgód. Ścieżkę zgody pokrywa `app-gui`.
    assert_eq!(status.actions.len(), 4, "{status:#?}");
    assert!(
        status
            .actions
            .iter()
            .all(|x| x.status == GuiActionStatus::Denied),
        "{status:#?}"
    );
    let kernel = run
        .steps
        .iter()
        .filter(|x| x.output.contains("blokada Jądra"))
        .count();
    assert_eq!(kernel, 2, "okna Alfy: blokada Jądra w Replay");
    let shown = serde_json::to_string(&status).unwrap();
    assert!(!shown.contains("mleko"), "panel bez wpisywanej treści");
    let gui_events: Vec<&AlfaEvent> = events
        .iter()
        .filter(|e| matches!(e, AlfaEvent::GuiActivity { .. }))
        .collect();
    assert!(!gui_events.is_empty(), "GuiActivity dla paska tytułu");
    assert!(
        !serde_json::to_string(&gui_events)
            .unwrap()
            .contains("mleko")
    );

    let stopped = a.h.core.gui_stop().await.unwrap();
    assert!(stopped.taken_over && stopped.control.is_none());
    let released = a.h.core.gui_release().await.unwrap();
    assert!(!released.taken_over);
}

#[derive(Default)]
struct Frames(Mutex<Vec<TerminalFrame>>);

impl FrameSink for Frames {
    fn send(&self, frame: TerminalFrame) {
        self.0.lock().unwrap().push(frame);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn terminal_input_only_by_command_and_never_in_events() {
    let d = desk();
    let pty = Arc::new(FakePty::with_banner(b"PS> ", true));
    let mut a = world(&d, pty.clone()).await;
    let frames = Arc::new(Frames::default());
    let session =
        a.h.core
            .terminal_open(TerminalProfileId::Shell, 100, 30, None, frames.clone())
            .await
            .unwrap();
    assert!(session.alive);
    // base64(SECRET + "\r") — bez zależności od crate'a base64 w testach.
    let b64 = "TGlzdGEgemFrdXBvdyBtbGVrbyA0NDE3DQ==".to_owned();
    a.h.core.terminal_input(session.id, b64).await.unwrap();
    a.h.core.terminal_resize(session.id, 120, 40).await.unwrap();
    assert_eq!(a.h.core.terminal_list().await.unwrap().len(), 1);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        pty.input_of(session.pid),
        format!("{SECRET}\r").into_bytes()
    );
    a.h.core.terminal_close(session.id).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        frames
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|f| matches!(f, TerminalFrame::Exit { .. }))
    );
    assert!(
        a.h.core
            .terminal_input(session.id, "eA==".into())
            .await
            .is_err()
    );
    let mut seen = Vec::new();
    while let Ok(batch) = a.h.rx.try_recv() {
        seen.extend(batch.iter().cloned());
    }
    let seen = serde_json::to_string(&seen).unwrap();
    assert!(!seen.contains("4417"), "treść terminala nie w zdarzeniach");
    assert!(!seen.contains("PS> "));
}

fn skill_manifest() -> serde_json::Value {
    json!({
        "id": "porzadki-pobranych",
        "version": "1.0.0",
        "name": "Porządki w Pobranych",
        "description": "Sortuje folder Pobrane według typu, bez usuwania plików.",
        "keywords": ["pobrane", "sortowanie"],
        "required_tools": ["fs_list", "fs_move"],
        "required_capabilities": ["fs.read", "fs.write"],
        "parameters": {
            "type": "object",
            "properties": {"folder": {"type": "string", "maxLength": 200}},
            "required": ["folder"],
            "additionalProperties": false
        },
        "prompt": "Uporządkuj folder {{folder}} według typu.",
        "acceptance": [
            {"name": "folder w celu", "params": {"folder": "C:/Users/ala/Downloads"},
             "expect_in_goal": ["C:/Users/ala/Downloads"]},
            {"name": "brak folderu", "params": {}, "expect_rejected": true}
        ]
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skill_proposal_review_approval_and_run_as_task() {
    let d = desk();
    let a = world(&d, Arc::new(FakePty::default())).await;
    let core = &a.h.core;
    let proposed = core.skills_propose(skill_manifest()).await.unwrap();
    assert_eq!(proposed.state, SkillStateView::Proposed);
    let review = core
        .skills_review(proposed.id.clone(), proposed.version.clone())
        .await
        .unwrap();
    assert!(!review.diff.is_empty());
    assert!(
        core.skills_approve(
            proposed.id.clone(),
            proposed.version.clone(),
            "0".repeat(64)
        )
        .await
        .is_err(),
        "instalacja tylko przejrzanej wersji"
    );
    let installed = core
        .skills_approve(
            proposed.id.clone(),
            proposed.version.clone(),
            review.skill.hash.clone(),
        )
        .await
        .unwrap();
    assert_eq!(installed.state, SkillStateView::Installed);
    assert!(
        core.skills_run(proposed.id.clone(), a.sid.clone(), None, json!({}))
            .await
            .is_err(),
        "parametry walidowane schematem"
    );
    let task = core
        .skills_run(
            proposed.id.clone(),
            a.sid.clone(),
            Some("delta".into()),
            json!({"folder": "C:/Users/ala/Downloads"}),
        )
        .await
        .unwrap();
    assert!(task.title.contains("Porządki"), "{task:?}");
    let listed = core.skills_list().await.unwrap();
    assert!(listed.iter().any(|s| s.id == proposed.id));
}

fn draft() -> AgentDraft {
    serde_json::from_value(json!({
        "id": null, "name": "Zofia", "forms": null, "glyph": "Z",
        "color": "color.agent.custom-2", "character": "Spokojna, konkretna.",
        "voice": {"base": "pl-f2", "pitch": 1.08, "rate": 1.0, "perceived_age": 22,
                  "timbre": "ciepła",
                  "design_prompt": "młoda dorosła kobieta, ok. 22 lat, ciepła, czysta polszczyzna"},
        "role": {"id": "porzadkowa", "name": "Porządkowa", "description": "Porządkuje Pobrane.",
                 "prompt": "Porządkuję pliki i mówię, co zrobiłam.", "model_policy": "conversation",
                 "tools": ["fs"], "read_only": false, "untrusted_isolated": false, "author": false},
        "limits": {"autonomy": "L2", "budget": null,
                   "fs_write": ["%USERPROFILE%\\Downloads\\**"], "memory_scope": "agent",
                   "retain_days": 30, "triggers": []},
        "skills": []
    }))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn builder_saves_only_after_dry_run_of_the_same_hash() {
    let d = desk();
    let a = world(&d, Arc::new(FakePty::default())).await;
    let core = &a.h.core;
    let policy = core.builder_policy().await.unwrap();
    assert!(policy.groups.iter().any(|g| g == "fs"));
    let preview = core.builder_preview(draft()).await.unwrap();
    assert_eq!(preview.forms.len(), 7);
    assert!(
        core.builder_save(draft(), preview.hash.clone())
            .await
            .is_err(),
        "zapis bez testu na sucho odrzucony"
    );
    let dry = core.builder_dry_run(draft()).await.unwrap();
    assert!(dry.passed, "{dry:#?}");
    assert_eq!(dry.hash, preview.hash);
    let saved = core.builder_save(draft(), dry.hash).await.unwrap();
    assert_eq!(saved.persona, preview.persona_id);
    let library = core.builder_library().await.unwrap();
    assert!(library.iter().any(|x| x.persona == saved.persona));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn provider_401_becomes_incident_repair_and_undo() {
    let d = desk();
    let mut a = world(&d, Arc::new(FakePty::default())).await;
    let core = a.h.core.clone();
    a.h.provider.push(Script::error(
        ProviderError::new(ProviderErrorKind::Auth, "Klucz odwołany.").with_status(401),
    ));
    let sent = core
        .turns_send(a.sid.clone(), send("Cześć", None))
        .await
        .unwrap();
    let _ = until(&mut a.h.rx, ends(&sent.assistant_turn_id.unwrap())).await;
    // „Sprawdź teraz” (skan okresowy działa w tle co kilkadziesiąt sekund).
    let mut view = core.health_scan().await.unwrap();
    for _ in 0..200 {
        if view.incidents.iter().any(|i| i.target == "fake") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
        view = core.health_scan().await.unwrap();
    }
    let incident = view
        .incidents
        .iter()
        .find(|i| i.target == "fake")
        .cloned()
        .unwrap_or_else(|| panic!("incydent klucza dostawcy: {view:#?}"));
    let id = incident.id;
    // Domyślna polityka naprawia niskie ryzyko sama (z „Cofnij”); inaczej — zgoda w UI.
    if let Some(card) = view.pending.iter().find(|p| p.id == id) {
        assert!(!card.kernel && !card.diff.is_empty(), "{card:#?}");
        view = core.health_approve(id).await.unwrap();
    }
    let journal = std::fs::read_to_string(core.paths().local.join("diagnostyka/journal.ndjson"))
        .unwrap_or_default();
    assert!(
        view.repaired.iter().any(|r| r.id == id && r.undoable),
        "{view:#?}\n{journal}"
    );
    let undone = core.health_undo(id).await.unwrap();
    assert!(!undone.repaired.iter().any(|r| r.id == id && r.undoable));
    assert!(
        core.health_undo(id).await.is_err(),
        "drugie cofnięcie odrzucone"
    );
}
