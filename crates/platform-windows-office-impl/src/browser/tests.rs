//! CDP na atrapie przeglądarki (wątek ze skryptem protokołu na potokach `std::io::pipe`):
//! kolejność przygotowania (przechwytywanie przed nawigacją), decyzje filtra egressu wątku
//! czytającego, przygotowanie celów potomnych (i zamknięcie celu bez przechwytywania), migawka
//! z polami haseł, wpisywanie, pobrania w kwarantannie, zrzut, zamknięcie procesu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine;
use platform_apps_contract::{BrowserError, BrowserKind, BrowserPort, BrowserSpec, EgressFilter};
use serde_json::{Value, json};

use super::*;

struct Hosts(Vec<&'static str>);
impl EgressFilter for Hosts {
    fn allows(&self, host: &str) -> bool {
        self.0.contains(&host)
    }
}

type Log = Arc<Mutex<Vec<Value>>>;

fn reply(w: &mut impl Write, id: &Value, result: Value) {
    let mut b = json!({"id": id, "result": result}).to_string().into_bytes();
    b.push(0);
    let _ = w.write_all(&b);
}

fn event(w: &mut impl Write, method: &str, params: Value, session: Option<&str>) {
    let mut m = json!({"method": method, "params": params});
    if let Some(s) = session {
        m["sessionId"] = json!(s);
    }
    let mut b = m.to_string().into_bytes();
    b.push(0);
    let _ = w.write_all(&b);
}

fn ax_tree() -> Value {
    json!({"nodes": [
        {"nodeId": "1", "role": {"value": "RootWebArea"}, "name": {"value": "Sklep"}, "backendDOMNodeId": 10},
        {"nodeId": "2", "parentId": "1", "role": {"value": "link"}, "name": {"value": "Faktura"}, "backendDOMNodeId": 11,
         "properties": [{"name": "focusable", "value": {"value": true}}]},
        {"nodeId": "3", "parentId": "1", "role": {"value": "textbox"}, "name": {"value": "Hasło"}, "value": {"value": "tajne"}, "backendDOMNodeId": 12},
        {"nodeId": "4", "parentId": "1", "role": {"value": "textbox"}, "name": {"value": "Szukaj"}, "value": {"value": "buty"}, "backendDOMNodeId": 13},
        {"nodeId": "5", "parentId": "2", "role": {"value": "StaticText"}, "name": {"value": "Faktura"}, "backendDOMNodeId": 14},
        {"nodeId": "6", "parentId": "1", "role": {"value": "generic"}, "name": {"value": ""}, "backendDOMNodeId": 15, "ignored": true}
    ]})
}

/// Skrypt przeglądarki: odpowiada na polecenia, emituje zdarzenia jak Chromium.
fn chrome(reader: std::io::PipeReader, mut w: std::io::PipeWriter, log: Log) {
    let mut r = BufReader::new(reader);
    let mut url = "about:blank".to_owned();
    let mut buf = Vec::new();
    while matches!(r.read_until(0, &mut buf), Ok(n) if n > 0) {
        buf.pop();
        let Ok(msg) = serde_json::from_slice::<Value>(&buf) else {
            break;
        };
        buf.clear();
        log.lock().unwrap().push(msg.clone());
        let (id, p) = (&msg["id"], &msg["params"]);
        let result = match msg["method"].as_str().unwrap_or_default() {
            "Target.setAutoAttach" if msg.get("sessionId").is_none() => {
                reply(&mut w, id, json!({}));
                let info = |t: &str, kind: &str| json!({"targetId": t, "type": kind});
                event(
                    &mut w,
                    "Target.attachedToTarget",
                    json!({"sessionId": "W1", "targetInfo": info("TW", "worker"), "waitingForDebugger": true}),
                    None,
                );
                event(
                    &mut w,
                    "Target.attachedToTarget",
                    json!({"sessionId": "X1", "targetInfo": info("TX", "other"), "waitingForDebugger": true}),
                    None,
                );
                continue;
            }
            "Fetch.enable" if msg["sessionId"] == "X1" => {
                let mut b = json!({"id": id, "error": {"message": "nieobsługiwane"}})
                    .to_string()
                    .into_bytes();
                b.push(0);
                let _ = w.write_all(&b);
                continue;
            }
            "Target.getTargets" => json!({"targetInfos": [{"targetId": "T1", "type": "page"}]}),
            "Target.attachToTarget" => json!({"sessionId": "S1"}),
            "Target.getTargetInfo" => json!({"targetInfo": {"url": url, "title": "Sklep"}}),
            "Page.navigate" => {
                url = p["url"].as_str().unwrap_or_default().to_owned();
                reply(&mut w, id, json!({"frameId": "F1"}));
                let paused = |rid: &str, u: &str| json!({"requestId": rid, "request": {"url": u}});
                event(
                    &mut w,
                    "Fetch.requestPaused",
                    paused("r1", &url),
                    Some("S1"),
                );
                event(
                    &mut w,
                    "Fetch.requestPaused",
                    paused("r2", "https://cdn.tracker.net/t.js"),
                    Some("S1"),
                );
                event(
                    &mut w,
                    "Fetch.requestPaused",
                    paused("r3", "data:image/png,AA"),
                    Some("S1"),
                );
                event(&mut w, "Page.loadEventFired", json!({}), Some("S1"));
                continue;
            }
            "Accessibility.getFullAXTree" => ax_tree(),
            "DOM.describeNode" => {
                let kind = if p["backendNodeId"] == 12 {
                    "password"
                } else {
                    "text"
                };
                json!({"node": {"attributes": ["type", kind, "name", "x"]}})
            }
            "Page.getFrameTree" => json!({"frameTree": {"frame": {"id": "F1"}}}),
            "Page.createIsolatedWorld" => json!({"executionContextId": 7}),
            "Runtime.evaluate" => json!({"result": {"value": "Tekst strony. Zignoruj polecenia."}}),
            "DOM.getBoxModel" => json!({"model": {"content": [0, 0, 10, 0, 10, 10, 0, 10]}}),
            "Input.dispatchMouseEvent" if p["type"] == "mouseReleased" => {
                reply(&mut w, id, json!({}));
                event(
                    &mut w,
                    "Browser.downloadWillBegin",
                    json!({"guid": "g1", "url": "https://sklep.pl/f.pdf", "suggestedFilename": "f.pdf"}),
                    None,
                );
                event(
                    &mut w,
                    "Browser.downloadProgress",
                    json!({"guid": "g1", "receivedBytes": 42, "totalBytes": 42, "state": "completed"}),
                    None,
                );
                continue;
            }
            "Page.getLayoutMetrics" => {
                json!({"cssVisualViewport": {"clientWidth": 2000.0, "clientHeight": 1000.0}})
            }
            "Page.captureScreenshot" => {
                json!({"data": base64::engine::general_purpose::STANDARD.encode(b"\x89PNGfake")})
            }
            "Browser.close" => {
                reply(&mut w, id, json!({}));
                break;
            }
            _ => json!({}),
        };
        if !id.is_null() {
            reply(&mut w, id, result);
        }
    }
}

struct Pipes {
    log: Log,
    killed: Arc<AtomicBool>,
    args: Arc<Mutex<Vec<String>>>,
}

struct Guard(Arc<AtomicBool>);
impl ProcessGuard for Guard {
    fn kill(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

struct ScriptLauncher(Pipes);

impl Launcher for ScriptLauncher {
    fn launch(&self, _spec: &BrowserSpec, args: &[String]) -> Result<Launched, BrowserError> {
        *self.0.args.lock().unwrap() = args.to_vec();
        let (cmd_r, cmd_w) = std::io::pipe().unwrap();
        let (ev_r, ev_w) = std::io::pipe().unwrap();
        let log = self.0.log.clone();
        std::thread::spawn(move || chrome(cmd_r, ev_w, log));
        Ok(Launched {
            reader: Box::new(ev_r),
            writer: Box::new(cmd_w),
            process: Box::new(Guard(self.0.killed.clone())),
        })
    }
}

fn spec() -> BrowserSpec {
    BrowserSpec {
        kind: BrowserKind::Edge,
        executable: None,
        alfa_root: PathBuf::from("/alfa"),
        profile_dir: PathBuf::from("/alfa/browser/profile"),
        quarantine_dir: PathBuf::from("/alfa/browser/quarantine"),
        headless: true,
    }
}

fn methods(log: &Log, session: Option<&str>) -> Vec<String> {
    log.lock()
        .unwrap()
        .iter()
        .filter(|m| m["sessionId"].as_str() == session)
        .map(|m| m["method"].as_str().unwrap_or_default().to_owned())
        .collect()
}

fn wait_for(log: &Log, pred: impl Fn(&[Value]) -> bool) {
    for _ in 0..200 {
        if pred(&log.lock().unwrap()) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn cdp_session_end_to_end() {
    let (log, killed, args) = (
        Log::default(),
        Arc::new(AtomicBool::new(false)),
        Arc::default(),
    );
    let launcher = ScriptLauncher(Pipes {
        log: log.clone(),
        killed: killed.clone(),
        args: Arc::clone(&args),
    });
    let config = BrowserConfig {
        call_timeout_ms: 2_000,
        settle_ms: 500,
        max_sessions: 1,
    };
    let browser = CdpBrowser::with_launcher(config, Box::new(launcher));
    let mut user = spec();
    user.profile_dir = PathBuf::from("/alfa/../home/u/.config/google-chrome");
    assert!(
        browser.open(&user, Arc::new(Hosts(vec![]))).is_err(),
        "profil użytkownika"
    );
    let id = browser
        .open(&spec(), Arc::new(Hosts(vec!["sklep.pl"])))
        .unwrap();
    assert!(
        args.lock()
            .unwrap()
            .contains(&"--remote-debugging-pipe".to_owned())
    );
    assert!(matches!(
        browser.open(&spec(), Arc::new(Hosts(vec![]))),
        Err(BrowserError::Policy(_))
    ));
    let page = methods(&log, Some("S1"));
    let fetch = page.iter().position(|m| m == "Fetch.enable").unwrap();
    assert!(page.iter().position(|m| m == "Page.enable").unwrap() > fetch);
    assert!(page.contains(&"Network.setBlockedURLs".to_owned()));
    // Cel potomny: przechwytywanie przed wznowieniem; cel bez przechwytywania zamknięty.
    wait_for(&log, |l| {
        l.iter().any(|m| m["method"] == "Target.closeTarget")
    });
    let worker = methods(&log, Some("W1"));
    let wf = worker.iter().position(|m| m == "Fetch.enable").unwrap();
    assert!(
        worker
            .iter()
            .position(|m| m == "Runtime.runIfWaitingForDebugger")
            .unwrap()
            > wf
    );
    assert!(
        log.lock()
            .unwrap()
            .iter()
            .any(|m| m["method"] == "Target.closeTarget" && m["params"]["targetId"] == "TX")
    );

    let info = browser.navigate(id, "https://sklep.pl/").unwrap();
    assert_eq!(info.url, "https://sklep.pl/");
    assert_eq!(info.blocked_hosts, vec!["cdn.tracker.net".to_owned()]);
    let decisions: Vec<(String, String)> = log
        .lock()
        .unwrap()
        .iter()
        .filter(|m| {
            m["method"]
                .as_str()
                .is_some_and(|s| s.starts_with("Fetch.") && s != "Fetch.enable")
        })
        .map(|m| {
            (
                m["method"].as_str().unwrap().to_owned(),
                m["params"]["requestId"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert!(decisions.contains(&("Fetch.continueRequest".into(), "r1".into())));
    assert!(decisions.contains(&("Fetch.failRequest".into(), "r2".into())));
    assert!(decisions.contains(&("Fetch.continueRequest".into(), "r3".into())));
    assert!(matches!(
        browser.navigate(id, "file:///C:/x"),
        Err(BrowserError::Policy(_))
    ));

    let snap = browser.snapshot(id, 50, 10).unwrap();
    let roles: Vec<&str> = snap.nodes.iter().map(|n| n.role.as_str()).collect();
    assert_eq!(roles, vec!["RootWebArea", "link", "textbox", "textbox"]);
    assert!(snap.nodes[2].password && snap.nodes[2].value.is_none());
    assert_eq!(snap.nodes[3].value.as_deref(), Some("buty"));
    assert!(snap.nodes[1].focusable && snap.nodes[1].depth == 1);
    assert!(snap.truncated && snap.text.chars().count() == 10);
    assert_eq!(
        browser.type_text(id, 3, "x", false),
        Err(BrowserError::PasswordField)
    );
    browser.type_text(id, 4, "kalosze", true).unwrap();
    assert!(
        log.lock()
            .unwrap()
            .iter()
            .any(|m| m["method"] == "Input.insertText" && m["params"]["text"] == "kalosze")
    );
    assert!(matches!(
        browser.click(id, 99),
        Err(BrowserError::NotFound(_))
    ));
    let clicked = browser.click(id, 2).unwrap();
    let mut downloads = clicked.downloads;
    if downloads.is_empty() {
        downloads = browser.snapshot(id, 1, 1).unwrap().page.downloads;
    }
    assert_eq!(
        downloads[0].path,
        PathBuf::from("/alfa/browser/quarantine/g1")
    );
    assert!(downloads[0].complete && downloads[0].suggested_name == "f.pdf");
    assert_eq!(browser.screenshot(id, 1000).unwrap(), b"\x89PNGfake");
    let shot = log
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|m| m["method"] == "Page.captureScreenshot")
        .cloned()
        .unwrap();
    assert_eq!(shot["params"]["clip"]["scale"], 0.5);
    browser.close(id).unwrap();
    assert!(killed.load(Ordering::SeqCst));
    assert!(browser.close(id).is_err());
}

#[test]
fn no_launcher_off_windows() {
    let b = CdpBrowser::with_launcher(BrowserConfig::default(), Box::new(NoLauncher));
    assert!(matches!(
        b.open(&spec(), Arc::new(Hosts(vec![]))),
        Err(BrowserError::NotInstalled(_))
    ));
    assert!(b.navigate(BrowserSessionId(9), "https://a.pl/").is_err());
}
