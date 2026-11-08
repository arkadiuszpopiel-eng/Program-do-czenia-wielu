//! Operacje narzędzi C6.4: przewijanie, klawisz, wybór z listy, historia karty (trzy ostatnie
//! z zgodą `net.egress(host strony)` jak kliknięcie), czekanie na tekst i konsola strony
//! (odczyt — wynik niezaufany, sekrety redagowane).

use std::time::{Duration, Instant};

use platform_apps_contract::{
    BrowserError, ConsoleLevel, HistoryNav, MAX_CONSOLE_ENTRIES, PageInfo,
};
use safety_broker_contract::TaintSource;
use tools_browser_contract::{
    ConsoleArgs, ConsoleOut, EVENT_ACT, KeyArgs, NavArgs, ScrollArgs, SelectArgs, WaitArgs, WaitOut,
};
use tools_common_contract::{
    ToolCtx, ToolManifest, ToolOutcome, parse_args, report_untrusted, text,
};

use super::{Core, Step};
use crate::out::{blocking, browser_failure};

/// Odstęp sprawdzania tekstu przy czekaniu.
const WAIT_POLL: Duration = Duration::from_millis(300);
/// Domyślne czekanie (ms).
const WAIT_DEFAULT_MS: u64 = 10_000;

impl Core {
    /// Akcja na stronie z zgodą na host bieżącej strony.
    async fn act_with<F>(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        action: &str,
        op: F,
    ) -> Step<ToolOutcome>
    where
        F: FnOnce(
                &dyn platform_apps_contract::BrowserPort,
                platform_apps_contract::BrowserSessionId,
            ) -> Result<PageInfo, BrowserError>
            + Send
            + 'static,
    {
        let b = self.current(ctx)?;
        let host = Self::page_host(&b)?;
        let action = &format!("{action} na stronie {}", b.url());
        let auths = self
            .egress(ctx, m, std::slice::from_ref(&host), action)
            .await?;
        let (port, id) = (self.browser.clone(), b.id);
        let r = blocking(move || op(port.as_ref(), id)).await?;
        match r {
            Ok(info) => {
                let event = serde_json::json!({"host": host, "action": m.name});
                Ok(self.finish(ctx, &auths, &b, info, (EVENT_ACT, event)).await)
            }
            Err(e) => {
                self.gate.release(&auths).await;
                Err(browser_failure(&e, action))
            }
        }
    }

    /// `browser_scroll` (bez Brokera — przewinięcie nie wysyła danych; żądania doczytywane
    /// przez stronę i tak przechodzą przez filtr hostów).
    pub(crate) async fn scroll(&self, args: serde_json::Value, ctx: &ToolCtx) -> Step<ToolOutcome> {
        let a: ScrollArgs = parse_args(args)?;
        let b = self.current(ctx)?;
        let (port, id) = (self.browser.clone(), b.id);
        let pixels = a.pixels.unwrap_or(600);
        let info = blocking(move || port.scroll(id, a.node, pixels))
            .await?
            .map_err(|e| browser_failure(&e, "przewinięcie strony"))?;
        Ok(self
            .finish(
                ctx,
                &[],
                &b,
                info,
                ("tool.browser.scroll", serde_json::json!({})),
            )
            .await)
    }

    /// `browser_key`.
    pub(crate) async fn key(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: KeyArgs = parse_args(args)?;
        let what = format!("naciśnięcie klawisza {:?}", a.key);
        self.act_with(ctx, m, &what, move |p, id| p.press_key(id, a.key))
            .await
    }

    /// `browser_select`.
    pub(crate) async fn select(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: SelectArgs = parse_args(args)?;
        let what = format!("wybór „{}” z listy (element nr {})", a.value, a.node);
        self.act_with(ctx, m, &what, move |p, id| {
            p.select_option(id, a.node, &a.value)
        })
        .await
    }

    /// `browser_nav`.
    pub(crate) async fn nav(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: NavArgs = parse_args(args)?;
        let what = match a.action {
            HistoryNav::Back => "cofnięcie do poprzedniej strony",
            HistoryNav::Forward => "przejście do następnej strony",
            HistoryNav::Reload => "odświeżenie strony",
        };
        self.act_with(ctx, m, what, move |p, id| p.history(id, a.action))
            .await
    }

    /// `browser_wait`: sprawdza tekst co 300 ms do limitu albo anulowania.
    pub(crate) async fn wait(&self, args: serde_json::Value, ctx: &ToolCtx) -> Step<ToolOutcome> {
        let a: WaitArgs = parse_args(args)?;
        let b = self.current(ctx)?;
        let limit = Duration::from_millis(a.timeout_ms.unwrap_or(WAIT_DEFAULT_MS));
        let start = Instant::now();
        let found = loop {
            let (port, id, needle) = (self.browser.clone(), b.id, a.text.clone());
            let hit = blocking(move || port.has_text(id, &needle))
                .await?
                .map_err(|e| browser_failure(&e, "czekanie na tekst"))?;
            if hit || start.elapsed() >= limit {
                break hit;
            }
            tokio::select! {
                () = ctx.cancel.cancelled() => return Ok(ToolOutcome::cancelled("czekanie na tekst")),
                () = tokio::time::sleep(WAIT_POLL) => {}
            }
        };
        report_untrusted(&self.gate, ctx, TaintSource::Web).await;
        let out = WaitOut {
            found,
            waited_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
        };
        let msg = if found {
            format!("Tekst pojawił się po {} ms.", out.waited_ms)
        } else {
            format!("Tekst nie pojawił się w {} ms.", out.waited_ms)
        };
        Ok(
            ToolOutcome::ok(msg, serde_json::to_value(&out).unwrap_or_default())
                .untrusted(TaintSource::Web),
        )
    }

    /// `browser_console`.
    pub(crate) async fn console(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
    ) -> Step<ToolOutcome> {
        let a: ConsoleArgs = parse_args(args)?;
        let b = self.current(ctx)?;
        let max = a.max.unwrap_or(100).clamp(1, MAX_CONSOLE_ENTRIES as u32) as usize;
        let (port, id) = (self.browser.clone(), b.id);
        let mut entries = blocking(move || port.console(id, max))
            .await?
            .map_err(|e| browser_failure(&e, "odczyt konsoli strony"))?;
        if a.errors_only.unwrap_or(false) {
            entries.retain(|e| e.level == ConsoleLevel::Error);
        }
        for e in &mut entries {
            e.text = text::redact_secrets(&e.text);
        }
        report_untrusted(&self.gate, ctx, TaintSource::Web).await;
        let errors = entries
            .iter()
            .filter(|e| e.level == ConsoleLevel::Error)
            .count() as u32;
        let lines: Vec<String> = entries
            .iter()
            .map(|e| {
                let place = match (&e.url, e.line) {
                    (Some(u), Some(l)) => format!(" ({u}:{l})"),
                    (Some(u), None) => format!(" ({u})"),
                    _ => String::new(),
                };
                format!("[{:?}/{}] {}{place}", e.level, e.source, e.text)
            })
            .collect();
        let body = if lines.is_empty() {
            "Konsola strony jest pusta od ostatniego odczytu.".to_owned()
        } else {
            format!("Błędy: {errors}. Wpisy (niezaufane):\n{}", lines.join("\n"))
        };
        let (body, cut) = text::truncate_chars(&body, self.config.output_max_chars);
        let out = ConsoleOut { entries, errors };
        let mut o = ToolOutcome::ok(body, serde_json::to_value(&out).unwrap_or_default())
            .untrusted(TaintSource::Web);
        o.truncated = cut;
        Ok(o)
    }
}
