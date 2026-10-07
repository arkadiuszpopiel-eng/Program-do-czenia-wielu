//! Strona przeglądarki przez CDP: przygotowanie (pobrania do kwarantanny, autodołączanie,
//! przechwytywanie żądań **przed** pierwszą nawigacją), nawigacja, drzewo dostępności z
//! wykrywaniem pól haseł (`DOM.describeNode`), tekst ze świata izolowanego, kliknięcie i
//! wpisywanie przez `Input.*` (zdarzenia zaufane), zrzut i stan strony (zablokowane hosty, pobrania).

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use base64::Engine;
use platform_apps_contract::{BrowserError, DownloadInfo, PageInfo, PageNode, PageSnapshot};
use serde_json::{Value, json};

use super::cdp::{ALWAYS_BLOCKED, Cdp, auto_attach, fetch_patterns};

/// Role pomijane w migawce (tekst strony jest osobno).
const SKIPPED_ROLES: [&str; 6] = [
    "StaticText",
    "InlineTextBox",
    "LineBreak",
    "none",
    "presentation",
    "ignored",
];
/// Role pól tekstowych (sprawdzane pod kątem haseł).
const TEXT_ROLES: [&str; 4] = ["textbox", "searchbox", "combobox", "spinbutton"];

/// Strona (jedna karta) z połączeniem.
pub(crate) struct Page {
    pub(crate) cdp: Cdp,
    target: String,
    session: String,
    quarantine: PathBuf,
    nodes: Mutex<Vec<i64>>,
    downloads: Mutex<BTreeMap<String, DownloadInfo>>,
    settle: Duration,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn proto(m: &str) -> BrowserError {
    BrowserError::Protocol(m.to_owned())
}

impl Page {
    /// Przygotowanie przeglądarki i pierwszej karty; błąd włączenia przechwytywania = odmowa.
    pub(crate) fn setup(
        cdp: Cdp,
        quarantine: PathBuf,
        settle: Duration,
    ) -> Result<Self, BrowserError> {
        let dir = quarantine.to_string_lossy().into_owned();
        cdp.send(
            "Browser.setDownloadBehavior",
            json!({"behavior": "allowAndName", "downloadPath": dir, "eventsEnabled": true}),
            None,
        )?;
        cdp.send("Target.setAutoAttach", auto_attach(), None)?;
        let targets = cdp.send("Target.getTargets", json!({}), None)?;
        let existing = targets["targetInfos"]
            .as_array()
            .and_then(|t| t.iter().find(|i| i["type"] == "page"))
            .and_then(|i| i["targetId"].as_str())
            .map(str::to_owned);
        let target = match existing {
            Some(t) => t,
            None => {
                cdp.send("Target.createTarget", json!({"url": "about:blank"}), None)?["targetId"]
                    .as_str()
                    .ok_or_else(|| proto("brak celu strony"))?
                    .to_owned()
            }
        };
        let attached = cdp.send(
            "Target.attachToTarget",
            json!({"targetId": target, "flatten": true}),
            None,
        )?;
        let session = attached["sessionId"]
            .as_str()
            .ok_or_else(|| proto("brak sesji celu"))?
            .to_owned();
        let s = Some(session.as_str());
        cdp.send("Fetch.enable", fetch_patterns(), s)?;
        cdp.send("Network.enable", json!({}), s)?;
        cdp.send("Network.setBlockedURLs", json!({"urls": ALWAYS_BLOCKED}), s)?;
        cdp.send("Target.setAutoAttach", auto_attach(), s)?;
        cdp.send("Page.enable", json!({}), s)?;
        let _ = cdp.send("Accessibility.enable", json!({}), s);
        let _ = cdp.send("Runtime.runIfWaitingForDebugger", json!({}), s);
        Ok(Self {
            cdp,
            target,
            session,
            quarantine,
            nodes: Mutex::new(Vec::new()),
            downloads: Mutex::new(BTreeMap::new()),
            settle,
        })
    }

    fn send(&self, method: &str, params: Value) -> Result<Value, BrowserError> {
        self.cdp.send(method, params, Some(&self.session))
    }

    fn mine(&self, e: &super::cdp::Event, method: &str) -> bool {
        e.method == method && e.session.as_deref() == Some(self.session.as_str())
    }

    fn collect_downloads(&self) -> Vec<DownloadInfo> {
        let events = self.cdp.drain(|e| e.method.starts_with("Browser.download"));
        let mut table = lock(&self.downloads);
        let mut touched: Vec<String> = Vec::new();
        for e in events {
            let Some(guid) = e.params["guid"].as_str() else {
                continue;
            };
            let entry = table
                .entry(guid.to_owned())
                .or_insert_with(|| DownloadInfo {
                    suggested_name: String::new(),
                    path: self.quarantine.join(guid),
                    bytes: 0,
                    complete: false,
                });
            if let Some(name) = e.params["suggestedFilename"].as_str() {
                entry.suggested_name = name.chars().take(255).collect();
            }
            if let Some(n) = e.params["receivedBytes"].as_u64() {
                entry.bytes = n;
            }
            entry.complete |= e.params["state"] == "completed";
            if !touched.iter().any(|g| g == guid) {
                touched.push(guid.to_owned());
            }
        }
        touched
            .iter()
            .filter_map(|g| table.get(g).cloned())
            .collect()
    }

    /// Stan strony: adres, tytuł, zablokowane hosty, pobrania.
    pub(crate) fn info(&self) -> Result<PageInfo, BrowserError> {
        let info = self.cdp.send(
            "Target.getTargetInfo",
            json!({"targetId": self.target}),
            None,
        )?;
        Ok(PageInfo {
            url: info["targetInfo"]["url"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            title: info["targetInfo"]["title"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(500)
                .collect(),
            blocked_hosts: self.cdp.take_blocked(),
            downloads: self.collect_downloads(),
        })
    }

    fn wait_load(&self, start_within: Option<Duration>) {
        if let Some(window) = start_within
            && self
                .cdp
                .wait_event(window, |e| self.mine(e, "Page.frameStartedLoading"))
                .is_none()
        {
            return;
        }
        let _ = self
            .cdp
            .wait_event(self.settle, |e| self.mine(e, "Page.loadEventFired"));
    }

    /// Nawigacja (host sprawdził już wywołujący; przekierowania przechodzą przez filtr).
    pub(crate) fn navigate(&self, url: &str) -> Result<PageInfo, BrowserError> {
        self.cdp.drain(|e| e.method.starts_with("Page."));
        let r = self.send("Page.navigate", json!({"url": url}))?;
        if let Some(err) = r["errorText"].as_str()
            && !err.contains("ERR_BLOCKED_BY_CLIENT")
            && !err.contains("ERR_ABORTED")
        {
            return Err(BrowserError::Protocol(format!("nawigacja: {err}")));
        }
        self.wait_load(None);
        self.info()
    }

    fn password(&self, backend: i64) -> Result<bool, BrowserError> {
        let d = self.send("DOM.describeNode", json!({"backendNodeId": backend}))?;
        let attrs: Vec<String> = d["node"]["attributes"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .map(str::to_lowercase)
                    .collect()
            })
            .unwrap_or_default();
        Ok(attrs
            .chunks(2)
            .any(|kv| kv.len() == 2 && kv[0] == "type" && kv[1] == "password"))
    }

    fn page_text(&self, max_text: usize) -> Result<(String, bool), BrowserError> {
        let tree = self.send("Page.getFrameTree", json!({}))?;
        let frame = tree["frameTree"]["frame"]["id"]
            .as_str()
            .ok_or_else(|| proto("brak ramki"))?
            .to_owned();
        let world = self.send(
            "Page.createIsolatedWorld",
            json!({"frameId": frame, "worldName": "alfa-odczyt"}),
        )?;
        let ctx = world["executionContextId"].clone();
        let r = self.send(
            "Runtime.evaluate",
            json!({"expression": "document.body ? document.body.innerText : ''",
                   "contextId": ctx, "returnByValue": true}),
        )?;
        let text = r["result"]["value"].as_str().unwrap_or_default();
        let cut = text.chars().count() > max_text;
        Ok((text.chars().take(max_text).collect(), cut))
    }

    /// Migawka: węzły dostępności (numerowane od 1) i tekst strony.
    pub(crate) fn snapshot(
        &self,
        max_nodes: usize,
        max_text: usize,
    ) -> Result<PageSnapshot, BrowserError> {
        let tree = self.send("Accessibility.getFullAXTree", json!({}))?;
        let all = tree["nodes"].as_array().cloned().unwrap_or_default();
        let mut depth: HashMap<String, u16> = HashMap::new();
        let (mut nodes, mut backends, mut truncated) = (Vec::new(), Vec::new(), false);
        for n in &all {
            let id = n["nodeId"].as_str().unwrap_or_default().to_owned();
            let d = n["parentId"]
                .as_str()
                .and_then(|p| depth.get(p))
                .map_or(0, |d| d.saturating_add(1));
            depth.insert(id, d);
            let role = n["role"]["value"].as_str().unwrap_or_default();
            let name = n["name"]["value"].as_str().unwrap_or_default();
            let Some(backend) = n["backendDOMNodeId"].as_i64() else {
                continue;
            };
            if n["ignored"] == true
                || SKIPPED_ROLES.contains(&role)
                || (role == "generic" && name.is_empty())
            {
                continue;
            }
            if nodes.len() >= max_nodes {
                truncated = true;
                break;
            }
            let password = TEXT_ROLES.contains(&role) && self.password(backend).unwrap_or(true);
            let focusable = n["properties"].as_array().is_some_and(|p| {
                p.iter()
                    .any(|x| x["name"] == "focusable" && x["value"]["value"] == true)
            });
            backends.push(backend);
            nodes.push(PageNode {
                node: u32::try_from(nodes.len() + 1).unwrap_or(u32::MAX),
                depth: d,
                role: role.to_owned(),
                name: name.chars().take(500).collect(),
                value: if password {
                    None
                } else {
                    n["value"]["value"]
                        .as_str()
                        .map(|v| v.chars().take(2_000).collect())
                },
                password,
                focusable,
            });
        }
        *lock(&self.nodes) = backends;
        let (text, cut) = self.page_text(max_text)?;
        Ok(PageSnapshot {
            page: self.info()?,
            nodes,
            text,
            truncated: truncated || cut,
        })
    }

    fn backend(&self, node: u32) -> Result<i64, BrowserError> {
        let idx = usize::try_from(node).ok().and_then(|n| n.checked_sub(1));
        idx.and_then(|i| lock(&self.nodes).get(i).copied())
            .ok_or_else(|| BrowserError::NotFound(format!("węzeł {node} — odśwież migawkę strony")))
    }

    /// Kliknięcie środka elementu (zdarzenia myszy CDP).
    pub(crate) fn click(&self, node: u32) -> Result<PageInfo, BrowserError> {
        let backend = self.backend(node)?;
        let _ = self.send(
            "DOM.scrollIntoViewIfNeeded",
            json!({"backendNodeId": backend}),
        );
        let model = self.send("DOM.getBoxModel", json!({"backendNodeId": backend}))?;
        let quad: Vec<f64> = model["model"]["content"]
            .as_array()
            .map(|q| q.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default();
        if quad.len() < 8 {
            return Err(BrowserError::NotFound(format!(
                "węzeł {node} nie jest widoczny"
            )));
        }
        let x = (quad[0] + quad[2] + quad[4] + quad[6]) / 4.0;
        let y = (quad[1] + quad[3] + quad[5] + quad[7]) / 4.0;
        self.cdp.drain(|e| e.method.starts_with("Page."));
        for kind in ["mouseMoved", "mousePressed", "mouseReleased"] {
            self.send(
                "Input.dispatchMouseEvent",
                json!({"type": kind, "x": x, "y": y, "button": "left", "clickCount": 1}),
            )?;
        }
        self.wait_load(Some(Duration::from_millis(700)));
        self.info()
    }

    /// Wpisanie tekstu (nigdy w pole hasła), opcjonalnie Enter.
    pub(crate) fn type_text(
        &self,
        node: u32,
        text: &str,
        submit: bool,
    ) -> Result<PageInfo, BrowserError> {
        let backend = self.backend(node)?;
        if self.password(backend)? {
            return Err(BrowserError::PasswordField);
        }
        self.send("DOM.focus", json!({"backendNodeId": backend}))?;
        self.send("Input.insertText", json!({"text": text}))?;
        if submit {
            self.cdp.drain(|e| e.method.starts_with("Page."));
            for kind in ["keyDown", "keyUp"] {
                self.send(
                    "Input.dispatchKeyEvent",
                    json!({"type": kind, "key": "Enter", "code": "Enter", "text": "\r",
                           "windowsVirtualKeyCode": 13, "nativeVirtualKeyCode": 13}),
                )?;
            }
            self.wait_load(Some(Duration::from_millis(700)));
        }
        self.info()
    }

    /// Zrzut widoku (PNG), dłuższy bok ≤ `max_side`.
    pub(crate) fn screenshot(&self, max_side: u32) -> Result<Vec<u8>, BrowserError> {
        let m = self.send("Page.getLayoutMetrics", json!({}))?;
        let vp = &m["cssVisualViewport"];
        let (w, h) = (
            vp["clientWidth"].as_f64().unwrap_or(1280.0).max(1.0),
            vp["clientHeight"].as_f64().unwrap_or(800.0).max(1.0),
        );
        let scale = (f64::from(max_side.max(64)) / w.max(h)).min(1.0);
        let shot = self.send(
            "Page.captureScreenshot",
            json!({"format": "png", "clip": {"x": 0, "y": 0, "width": w, "height": h, "scale": scale}}),
        )?;
        let data = shot["data"]
            .as_str()
            .ok_or_else(|| proto("brak danych zrzutu"))?;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| proto(&format!("zrzut: {e}")))
    }

    /// Zamknięcie przeglądarki (proces dobija właściciel — Job Object).
    pub(crate) fn close(&self) {
        let _ = self.cdp.send("Browser.close", json!({}), None);
    }
}
