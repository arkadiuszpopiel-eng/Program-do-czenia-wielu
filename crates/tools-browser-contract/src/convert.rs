//! Walidacja argumentów (ta sama w `-impl` i `-fake`), normalizacja hostów i tekst migawki.

use platform_apps_contract::{MAX_TYPE_CHARS, check_navigation_url, url_host};

use crate::{ClickArgs, CloseArgs, NodeOut, OpenArgs, ReadArgs, ShotArgs, TypeArgs};

/// Host od modelu (`cdn.example.com` albo adres) → postać kanoniczna; `None` dla wzorców
/// wieloznacznych, pustych i niepoprawnych.
pub fn normalize_host(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.contains(['*', ' ', '@']) {
        return None;
    }
    let url = if t.contains("://") {
        t.to_owned()
    } else {
        format!("https://{t}")
    };
    url_host(&url).filter(|h| h.contains('.') || h == "localhost")
}

/// Sprawdza argumenty narzędzia.
pub fn check_args(tool: &str, args: &serde_json::Value) -> Result<(), String> {
    let v = args.clone();
    let e = |e: serde_json::Error| e.to_string();
    match tool {
        "browser_open" => {
            let a: OpenArgs = serde_json::from_value(v).map_err(e)?;
            check_navigation_url(&a.url).map_err(|e| e.to_string())?;
            if a.extra_hosts.len() > 16 {
                return Err("najwyżej 16 dodatkowych hostów".into());
            }
            for h in &a.extra_hosts {
                normalize_host(h).ok_or_else(|| format!("niepoprawny host `{h}`"))?;
            }
            Ok(())
        }
        "browser_read" => serde_json::from_value::<ReadArgs>(v).map(|_| ()).map_err(e),
        "browser_click" => serde_json::from_value::<ClickArgs>(v)
            .map(|_| ())
            .map_err(e),
        "browser_type" => {
            let a: TypeArgs = serde_json::from_value(v).map_err(e)?;
            if a.text.chars().count() > MAX_TYPE_CHARS {
                return Err("tekst za długi".into());
            }
            Ok(())
        }
        "browser_screenshot" => serde_json::from_value::<ShotArgs>(v).map(|_| ()).map_err(e),
        "browser_close" => serde_json::from_value::<CloseArgs>(v)
            .map(|_| ())
            .map_err(e),
        other => Err(format!("nieznane narzędzie {other}")),
    }
}

/// Przykładowe poprawne argumenty.
pub fn sample_args(tool: &str) -> serde_json::Value {
    match tool {
        "browser_open" => serde_json::json!({"url": "https://example.com/"}),
        "browser_click" => serde_json::json!({"node": 1}),
        "browser_type" => serde_json::json!({"node": 1, "text": "szukaj"}),
        _ => serde_json::json!({}),
    }
}

/// Węzły jako tekst dla modelu (`[n] rola „nazwa” = wartość`).
pub fn render_nodes(nodes: &[NodeOut]) -> String {
    nodes
        .iter()
        .map(|n| {
            let indent = "  ".repeat(usize::from(n.depth.min(12)));
            let value = match (&n.value, n.password) {
                (_, true) => " = [pole hasła]".to_owned(),
                (Some(v), false) if !v.is_empty() => format!(" = „{v}”"),
                _ => String::new(),
            };
            format!("{indent}[{}] {} „{}”{value}", n.node, n.role, n.name)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
