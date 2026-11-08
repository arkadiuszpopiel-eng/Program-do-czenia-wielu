//! Operacje strony C6.4 przez CDP: przewijanie (kółko myszy albo przewinięcie do węzła), klawisze
//! specjalne (`Input.dispatchKeyEvent` — zdarzenia zaufane), wybór opcji `<select>` (funkcja na
//! elemencie wskazanym przez węzeł dostępności, ze zdarzeniami `input`/`change`), historia karty,
//! wyszukiwanie tekstu (świat izolowany) i konsola strony (wpisy zbierane przez połączenie).

use std::time::Duration;

use platform_apps_contract::{
    BrowserError, BrowserKey, ConsoleEntry, ConsoleLevel, HistoryNav, MAX_SCROLL_PX, PageInfo,
    console_level,
};
use serde_json::{Value, json};

use super::Page;
use crate::browser::cdp::Event;

/// Wybór opcji listy: po wartości albo widocznym tekście; zdarzenia jak przy wyborze myszą.
const SELECT_FN: &str = "function(v){if(this.tagName!=='SELECT')return 'not-select';\
const t=(s)=>(s||'').trim().toLowerCase();\
const o=[...this.options].find(o=>o.value===v||t(o.text)===t(v));\
if(!o)return 'no-option:'+[...this.options].map(o=>o.text.trim()).slice(0,30).join(', ');\
this.value=o.value;this.dispatchEvent(new Event('input',{bubbles:true}));\
this.dispatchEvent(new Event('change',{bubbles:true}));return 'ok';}";

/// Wpis konsoli ze zdarzenia CDP (`Runtime.consoleAPICalled`, `Runtime.exceptionThrown`,
/// `Log.entryAdded`).
pub(crate) fn console_entry(e: &Event) -> Option<ConsoleEntry> {
    let p = &e.params;
    let (level, source, text, url, line) = match e.method.as_str() {
        "Runtime.consoleAPICalled" => {
            let text = p["args"]
                .as_array()
                .map(|args| {
                    args.iter()
                        .map(|a| match &a["value"] {
                            Value::String(s) => s.clone(),
                            Value::Null => a["description"].as_str().unwrap_or("").to_owned(),
                            other => other.to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            let frame = &p["stackTrace"]["callFrames"][0];
            (
                console_level(p["type"].as_str().unwrap_or("log")),
                "console",
                text,
                frame["url"].as_str().map(str::to_owned),
                frame["lineNumber"].as_u64(),
            )
        }
        "Runtime.exceptionThrown" => {
            let d = &p["exceptionDetails"];
            let text = d["exception"]["description"]
                .as_str()
                .or_else(|| d["text"].as_str())
                .unwrap_or("wyjątek")
                .to_owned();
            (
                ConsoleLevel::Error,
                "exception",
                text,
                d["url"].as_str().map(str::to_owned),
                d["lineNumber"].as_u64(),
            )
        }
        "Log.entryAdded" => {
            let entry = &p["entry"];
            let source = if entry["source"] == "network" {
                "network"
            } else {
                "log"
            };
            (
                console_level(entry["level"].as_str().unwrap_or("info")),
                source,
                entry["text"].as_str().unwrap_or("").to_owned(),
                entry["url"].as_str().map(str::to_owned),
                entry["lineNumber"].as_u64(),
            )
        }
        _ => return None,
    };
    let mut out = ConsoleEntry::new(level, source, &text);
    out.url = url.filter(|u| !u.is_empty());
    // CDP liczy wiersze od 0.
    out.line = line.and_then(|l| u32::try_from(l + 1).ok());
    Some(out)
}

impl Page {
    /// Przewinięcie o `delta_y` px albo do węzła.
    pub(crate) fn scroll(&self, node: Option<u32>, delta_y: i32) -> Result<PageInfo, BrowserError> {
        match node {
            Some(n) => {
                let backend = self.backend(n)?;
                self.send(
                    "DOM.scrollIntoViewIfNeeded",
                    json!({"backendNodeId": backend}),
                )?;
            }
            None => {
                let m = self.send("Page.getLayoutMetrics", json!({}))?;
                let vp = &m["cssVisualViewport"];
                let x = vp["clientWidth"].as_f64().unwrap_or(800.0) / 2.0;
                let y = vp["clientHeight"].as_f64().unwrap_or(600.0) / 2.0;
                self.send(
                    "Input.dispatchMouseEvent",
                    json!({"type": "mouseWheel", "x": x, "y": y, "deltaX": 0,
                           "deltaY": delta_y.clamp(-MAX_SCROLL_PX, MAX_SCROLL_PX)}),
                )?;
            }
        }
        std::thread::sleep(Duration::from_millis(150));
        self.info()
    }

    /// Klawisz specjalny (Enter czeka na ewentualną nawigację).
    pub(crate) fn press_key(&self, key: BrowserKey) -> Result<PageInfo, BrowserError> {
        let (k, code, vk, text) = key.cdp();
        let modifiers = key.modifiers();
        self.cdp.drain(|e| e.method.starts_with("Page."));
        let mut down = json!({"type": if text.is_some() { "keyDown" } else { "rawKeyDown" },
            "key": k, "code": code, "windowsVirtualKeyCode": vk, "nativeVirtualKeyCode": vk,
            "modifiers": modifiers});
        if let Some(t) = text {
            down["text"] = json!(t);
            down["unmodifiedText"] = json!(t);
        }
        self.send("Input.dispatchKeyEvent", down)?;
        self.send(
            "Input.dispatchKeyEvent",
            json!({"type": "keyUp", "key": k, "code": code, "windowsVirtualKeyCode": vk,
                   "nativeVirtualKeyCode": vk, "modifiers": modifiers}),
        )?;
        if key == BrowserKey::Enter {
            self.wait_load(Some(Duration::from_millis(700)));
        }
        self.info()
    }

    /// Wybór opcji listy `<select>` wskazanej węzłem.
    pub(crate) fn select_option(&self, node: u32, value: &str) -> Result<PageInfo, BrowserError> {
        let backend = self.backend(node)?;
        let resolved = self.send("DOM.resolveNode", json!({"backendNodeId": backend}))?;
        let object = resolved["object"]["objectId"]
            .as_str()
            .ok_or_else(|| BrowserError::NotFound(format!("węzeł {node} — odśwież migawkę")))?
            .to_owned();
        let r = self.send(
            "Runtime.callFunctionOn",
            json!({"objectId": object, "functionDeclaration": SELECT_FN,
                   "arguments": [{"value": value}], "returnByValue": true}),
        )?;
        let _ = self.send("Runtime.releaseObject", json!({"objectId": object}));
        match r["result"]["value"].as_str().unwrap_or_default() {
            "ok" => {
                self.wait_load(Some(Duration::from_millis(300)));
                self.info()
            }
            "not-select" => Err(BrowserError::NotFound(format!(
                "węzeł {node} nie jest listą wyboru — kliknij go i wybierz opcję z menu strony"
            ))),
            other => Err(BrowserError::NotFound(format!(
                "lista {node} nie ma opcji „{value}” (dostępne: {})",
                other.trim_start_matches("no-option:")
            ))),
        }
    }

    /// Wstecz, dalej albo odśwież.
    pub(crate) fn history(&self, nav: HistoryNav) -> Result<PageInfo, BrowserError> {
        self.cdp.drain(|e| e.method.starts_with("Page."));
        if nav == HistoryNav::Reload {
            self.send("Page.reload", json!({}))?;
            self.wait_load(Some(Duration::from_millis(1_000)));
            return self.info();
        }
        let h = self.send("Page.getNavigationHistory", json!({}))?;
        let current = h["currentIndex"].as_i64().unwrap_or(0);
        let target = if nav == HistoryNav::Back {
            current - 1
        } else {
            current + 1
        };
        let entry = usize::try_from(target)
            .ok()
            .and_then(|i| h["entries"].get(i))
            .filter(|e| {
                !e["url"]
                    .as_str()
                    .unwrap_or("about:blank")
                    .starts_with("about:")
            })
            .ok_or_else(|| BrowserError::NotFound("brak strony w historii karty".into()))?;
        self.send(
            "Page.navigateToHistoryEntry",
            json!({"entryId": entry["id"]}),
        )?;
        self.wait_load(Some(Duration::from_millis(1_000)));
        self.info()
    }

    /// Czy tekst strony zawiera `text` (bez rozróżniania wielkości liter).
    pub(crate) fn has_text(&self, text: &str) -> Result<bool, BrowserError> {
        let (page, _) = self.page_text(500_000)?;
        Ok(page.to_lowercase().contains(&text.to_lowercase()))
    }

    /// Wpisy konsoli zebrane od ostatniego odczytu.
    pub(crate) fn console(&self, max: usize) -> Vec<ConsoleEntry> {
        self.cdp
            .take_console(&self.session, max)
            .iter()
            .filter_map(console_entry)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(method: &str, params: Value) -> Event {
        Event {
            method: method.into(),
            params,
            session: Some("S1".into()),
        }
    }

    #[test]
    fn console_events_become_entries() {
        let log = console_entry(&ev(
            "Runtime.consoleAPICalled",
            json!({"type": "warning", "args": [{"type": "string", "value": "uwaga"}, {"type": "number", "value": 3}],
                   "stackTrace": {"callFrames": [{"url": "http://localhost:5173/app.js", "lineNumber": 9}]}}),
        ))
        .unwrap();
        assert_eq!(log.level, ConsoleLevel::Warning);
        assert_eq!(log.text, "uwaga 3");
        assert_eq!(log.line, Some(10));
        let ex = console_entry(&ev(
            "Runtime.exceptionThrown",
            json!({"exceptionDetails": {"text": "Uncaught", "exception": {"description": "TypeError: x is undefined"},
                   "url": "http://localhost/a.js", "lineNumber": 0}}),
        ))
        .unwrap();
        assert_eq!(
            (ex.level, ex.source.as_str()),
            (ConsoleLevel::Error, "exception")
        );
        assert!(ex.text.starts_with("TypeError"));
        let net = console_entry(&ev(
            "Log.entryAdded",
            json!({"entry": {"source": "network", "level": "error", "text": "404 (Not Found)", "url": "http://localhost/x.png"}}),
        ))
        .unwrap();
        assert_eq!(net.source, "network");
        assert!(console_entry(&ev("Page.loadEventFired", json!({}))).is_none());
    }
}
