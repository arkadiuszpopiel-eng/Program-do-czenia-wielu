//! Wyniki narzędzi przeglądarki: mapowanie błędów portu na wyniki dla modelu, stan strony
//! (tytuł redagowany, zablokowane hosty, pobrania w kwarantannie), wywołania blokujące.

use platform_apps_contract::{BrowserError, PageInfo};
use tools_browser_contract::{DownloadOut, PageOut};
use tools_common_contract::{DenialReason, ToolErrorKind, ToolOutcome, text};

use crate::ops::Step;
use crate::session::HostAllow;

pub(crate) fn fail(kind: ToolErrorKind, text: String) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::failed(kind, text))
}

/// Błąd przeglądarki → wynik dla modelu.
pub(crate) fn browser_failure(e: &BrowserError, action: &str) -> Box<ToolOutcome> {
    let deny = |reason: DenialReason, hint: &str| {
        let mut o = ToolOutcome::denied(reason, action);
        o.text = format!("Odmowa: {action} — {e}. {hint}");
        Box::new(o)
    };
    match e {
        BrowserError::PasswordField => {
            deny(DenialReason::Policy, "Hasła wpisuje wyłącznie właściciel.")
        }
        BrowserError::Policy(_) => deny(DenialReason::Policy, "Wybierz inny adres lub inną drogę."),
        BrowserError::Blocked(_) => deny(
            DenialReason::Policy,
            "Użyj `browser_open` z tym adresem albo `extra_hosts`, żeby poprosić o zgodę.",
        ),
        BrowserError::NotFound(_) => fail(
            ToolErrorKind::NotFound,
            format!("Nie wykonano: {action} — {e}. Odśwież `browser_read`."),
        ),
        BrowserError::Timeout { .. } => fail(
            ToolErrorKind::Timeout,
            format!("Nie wykonano: {action} — {e}."),
        ),
        BrowserError::NotInstalled(_) => fail(
            ToolErrorKind::Unsupported,
            format!("Nie wykonano: {action} — {e}."),
        ),
        _ => fail(ToolErrorKind::Io, format!("Nie wykonano: {action} — {e}.")),
    }
}

pub(crate) async fn blocking<T, F>(work: F) -> Step<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| fail(ToolErrorKind::Internal, format!("Wątek przeglądarki: {e}.")))
}

pub(crate) fn page_out(p: &PageInfo, allow: &HostAllow) -> PageOut {
    PageOut {
        url: p.url.clone(),
        title: text::redact_secrets(&p.title),
        blocked_hosts: p.blocked_hosts.clone(),
        downloads: p
            .downloads
            .iter()
            .map(|d| DownloadOut {
                suggested_name: d.suggested_name.clone(),
                path: d.path.to_string_lossy().into_owned(),
                bytes: d.bytes,
                complete: d.complete,
            })
            .collect(),
        approved_hosts: allow.hosts(),
    }
}

pub(crate) fn page_text(p: &PageOut) -> String {
    let mut s = format!("Strona „{}” ({})", p.title, p.url);
    if !p.blocked_hosts.is_empty() {
        s.push_str(&format!(
            "\nZablokowane hosty (bez zgody na ruch sieciowy): {}.",
            p.blocked_hosts.join(", ")
        ));
    }
    for d in &p.downloads {
        s.push_str(&format!(
            "\nPobrano do kwarantanny: „{}” → {} ({} B) — plik niezaufany.",
            d.suggested_name, d.path, d.bytes
        ));
    }
    s
}
