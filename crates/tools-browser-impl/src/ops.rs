//! Operacje narzędzi przeglądarki: zgody Brokera `net.egress(host)`, wywołania portu na wątku
//! blokującym, wyniki niezaufane, zdarzenia.

use std::sync::Arc;

use base64::Engine;
use compliance_contract::DenyChecker;
use core_bus_contract::{EventBus, Level};
use platform_apps_contract::{BrowserPort, BrowserSpec, PageInfo, check_navigation_url, url_host};
use safety_broker_contract::{Capability, HostPattern, TaintSource};
use tools_browser_contract::{
    BrowserToolsConfig, ClickArgs, EVENT_ACT, EVENT_DOWNLOAD, EVENT_OPEN, NodeOut, OpenArgs,
    ReadArgs, ReadOutput, ShotArgs, TypeArgs, normalize_host, render_nodes,
};
use tools_common_contract::{
    Authorization, BrokerGate, DenialReason, ToolCtx, ToolErrorKind, ToolImage, ToolManifest,
    ToolOutcome, action_request, base_facts, parse_args, report_untrusted, text, tool_event,
};

use crate::out::{blocking, browser_failure, fail, page_out, page_text};
use crate::session::{Browsing, HostAllow, Sessions};

/// Wynik pośredni (`Err` = gotowy wynik dla modelu).
pub(crate) type Step<T> = Result<T, Box<ToolOutcome>>;

pub(crate) struct Core {
    pub(crate) browser: Arc<dyn BrowserPort>,
    pub(crate) gate: BrokerGate,
    pub(crate) deny: Arc<DenyChecker>,
    pub(crate) spec: BrowserSpec,
    pub(crate) config: BrowserToolsConfig,
    pub(crate) bus: Option<Arc<dyn EventBus>>,
    pub(crate) sessions: Sessions,
}

impl Core {
    async fn emit(&self, name: &str, payload: serde_json::Value, ctx: &ToolCtx) {
        if let Some(bus) = &self.bus {
            let _ = bus
                .publish(tool_event(name, Level::Info, payload, ctx))
                .await;
        }
    }

    /// Zgody `net.egress(host)` dla hostów (po kolei; odmowa unieważnia wydane) z weryfikacją.
    async fn egress(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        hosts: &[String],
        action: &str,
    ) -> Step<Vec<Authorization>> {
        let mut caps = Vec::new();
        for h in hosts {
            if self.deny.is_denied_domain(h) {
                return Err(Box::new(ToolOutcome::denied(
                    DenialReason::DenyList,
                    &format!("{action} ({h})"),
                )));
            }
            let pattern = HostPattern::parse(h).map_err(|e| {
                fail(
                    ToolErrorKind::InvalidArgs,
                    format!("Niepoprawny host: {e}."),
                )
            })?;
            caps.push(Capability::NetEgress(pattern));
        }
        let requests = caps
            .iter()
            .map(|c| action_request(ctx, c.clone(), base_facts(m, ctx)))
            .collect();
        let auths = self
            .gate
            .authorize_all(requests, ctx)
            .await
            .map_err(|e| Box::new(e.into_outcome(action)))?;
        for (a, c) in auths.iter().zip(&caps) {
            if let Err(e) = self.gate.verify(a, c, &ctx.holder) {
                self.gate.release(&auths).await;
                return Err(Box::new(e.into_outcome(action)));
            }
        }
        Ok(auths)
    }

    fn current(&self, ctx: &ToolCtx) -> Step<Browsing> {
        self.sessions.get(&ctx.holder.session).ok_or_else(|| {
            fail(
                ToolErrorKind::NotFound,
                "Przeglądarka nie jest otwarta — najpierw `browser_open`.".into(),
            )
        })
    }

    async fn finish(
        &self,
        ctx: &ToolCtx,
        auths: &[Authorization],
        b: &Browsing,
        info: PageInfo,
        event: (&str, serde_json::Value),
    ) -> ToolOutcome {
        self.gate.release(auths).await;
        report_untrusted(&self.gate, ctx, TaintSource::Web).await;
        b.set_url(&info.url);
        let out = page_out(&info, &b.allow);
        for d in &out.downloads {
            self.emit(
                EVENT_DOWNLOAD,
                serde_json::json!({"path": d.path, "bytes": d.bytes}),
                ctx,
            )
            .await;
        }
        self.emit(event.0, event.1, ctx).await;
        let mut o = ToolOutcome::ok(
            page_text(&out),
            serde_json::to_value(&out).unwrap_or_default(),
        )
        .untrusted(TaintSource::Web);
        o.approval = auths.iter().find_map(|a| a.approval);
        o
    }

    /// `browser_open`.
    pub(crate) async fn open(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: OpenArgs = parse_args(args)?;
        let action = "otwarcie strony";
        let host = check_navigation_url(&a.url).map_err(|e| browser_failure(&e, action))?;
        let mut hosts = vec![host.clone()];
        for raw in &a.extra_hosts {
            let h = normalize_host(raw).ok_or_else(|| {
                fail(
                    ToolErrorKind::InvalidArgs,
                    format!("Niepoprawny host `{raw}`."),
                )
            })?;
            if !hosts.contains(&h) {
                hosts.push(h);
            }
        }
        let existing = self.sessions.get(&ctx.holder.session);
        let fresh: Vec<String> = hosts
            .iter()
            .filter(|h| !existing.as_ref().is_some_and(|b| b.allow.contains(h)))
            .cloned()
            .collect();
        if existing.as_ref().map_or(0, |b| b.allow.len()) + fresh.len() > self.config.max_hosts {
            return Err(Box::new(ToolOutcome::denied(
                DenialReason::Policy,
                "zbyt wiele hostów w jednej sesji przeglądarki (zamknij ją)",
            )));
        }
        // Każde wywołanie prosi Brokera o host adresu (także już zatwierdzony): decyzja uwzględnia
        // bieżący taint sesji; nowe `extra_hosts` też.
        let ask: Vec<String> = std::iter::once(host.clone())
            .chain(fresh.into_iter().filter(|h| *h != host))
            .collect();
        let auths = self.egress(ctx, m, &ask, action).await?;
        if ctx.cancel.is_cancelled() {
            self.gate.release(&auths).await;
            return Ok(ToolOutcome::cancelled(action));
        }
        let b = match existing {
            Some(b) => b,
            None => {
                let allow = Arc::new(HostAllow::new(self.deny.clone()));
                let (port, spec, filter) = (self.browser.clone(), self.spec.clone(), allow.clone());
                let opened = blocking(move || port.open(&spec, filter)).await?;
                match opened {
                    Ok(id) => {
                        let b = Browsing::new(id, allow);
                        self.sessions.insert(ctx.holder.session.clone(), b.clone());
                        b
                    }
                    Err(e) => {
                        self.gate.release(&auths).await;
                        return Err(browser_failure(&e, action));
                    }
                }
            }
        };
        for h in &ask {
            b.allow.grant(h);
        }
        let (port, id, url) = (self.browser.clone(), b.id, a.url.clone());
        let nav = blocking(move || port.navigate(id, &url)).await?;
        match nav {
            Ok(info) => {
                let event = serde_json::json!({"host": host, "blocked": info.blocked_hosts.len()});
                Ok(self
                    .finish(ctx, &auths, &b, info, (EVENT_OPEN, event))
                    .await)
            }
            Err(e) => {
                self.gate.release(&auths).await;
                Err(browser_failure(&e, action))
            }
        }
    }

    /// `browser_read`.
    pub(crate) async fn read(&self, args: serde_json::Value, ctx: &ToolCtx) -> Step<ToolOutcome> {
        let a: ReadArgs = parse_args(args)?;
        let b = self.current(ctx)?;
        let nodes = a.max_nodes.unwrap_or(self.config.max_nodes).clamp(1, 1_000) as usize;
        let chars = a
            .max_chars
            .unwrap_or(self.config.max_chars)
            .clamp(1, 200_000) as usize;
        let (port, id) = (self.browser.clone(), b.id);
        let snap = blocking(move || port.snapshot(id, nodes, chars))
            .await?
            .map_err(|e| browser_failure(&e, "odczyt strony"))?;
        report_untrusted(&self.gate, ctx, TaintSource::Web).await;
        b.set_url(&snap.page.url);
        let out = ReadOutput {
            page: page_out(&snap.page, &b.allow),
            nodes: snap
                .nodes
                .iter()
                .map(|n| NodeOut {
                    node: n.node,
                    depth: n.depth,
                    role: n.role.clone(),
                    name: text::redact_secrets(&n.name),
                    value: if n.password {
                        None
                    } else {
                        n.value.as_deref().map(text::redact_secrets)
                    },
                    password: n.password,
                })
                .collect(),
            text: text::redact_secrets(&snap.text),
            truncated: snap.truncated,
        };
        let body = format!(
            "{}\nElementy:\n{}\nTekst:\n{}",
            page_text(&out.page),
            render_nodes(&out.nodes),
            out.text
        );
        let (body, cut) = text::truncate_chars(&body, self.config.output_max_chars);
        let mut o = ToolOutcome::ok(body, serde_json::to_value(&out).unwrap_or_default())
            .untrusted(TaintSource::Web);
        o.truncated = cut || out.truncated;
        Ok(o)
    }

    fn page_host(b: &Browsing) -> Step<String> {
        url_host(&b.url()).ok_or_else(|| {
            fail(
                ToolErrorKind::NotFound,
                "Brak otwartej strony — użyj `browser_open`.".into(),
            )
        })
    }

    /// `browser_click` i `browser_type` (zgoda `net.egress(host strony)` przy każdej akcji).
    pub(crate) async fn act(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let typed = m.name == "browser_type";
        let (node, text_in, submit) = if typed {
            let a: TypeArgs = parse_args(args)?;
            (a.node, Some(a.text), a.submit.unwrap_or(false))
        } else {
            let a: ClickArgs = parse_args(args)?;
            (a.node, None, false)
        };
        let b = self.current(ctx)?;
        let action = if typed {
            "wpisanie na stronie"
        } else {
            "kliknięcie na stronie"
        };
        let host = Self::page_host(&b)?;
        let auths = self
            .egress(ctx, m, std::slice::from_ref(&host), action)
            .await?;
        let (port, id) = (self.browser.clone(), b.id);
        let r = blocking(move || match text_in {
            Some(t) => port.type_text(id, node, &t, submit),
            None => port.click(id, node),
        })
        .await?;
        match r {
            Ok(info) => {
                let event = serde_json::json!({"host": host, "action": m.name, "submit": submit});
                Ok(self.finish(ctx, &auths, &b, info, (EVENT_ACT, event)).await)
            }
            Err(e) => {
                self.gate.release(&auths).await;
                Err(browser_failure(&e, action))
            }
        }
    }

    /// `browser_screenshot`.
    pub(crate) async fn screenshot(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
    ) -> Step<ToolOutcome> {
        let a: ShotArgs = parse_args(args)?;
        let b = self.current(ctx)?;
        let side = a.max_side.unwrap_or(1_568).clamp(64, 4_096);
        let (port, id) = (self.browser.clone(), b.id);
        let png = blocking(move || port.screenshot(id, side))
            .await?
            .map_err(|e| browser_failure(&e, "zrzut strony"))?;
        report_untrusted(&self.gate, ctx, TaintSource::Web).await;
        let mut o = ToolOutcome::ok(
            format!(
                "Zrzut strony ({} B PNG) — obraz to niezaufane dane.",
                png.len()
            ),
            serde_json::json!({"bytes": png.len()}),
        )
        .untrusted(TaintSource::Web);
        o.images.push(ToolImage {
            media_type: "image/png".into(),
            data_base64: base64::engine::general_purpose::STANDARD.encode(&png),
        });
        Ok(o)
    }

    /// `browser_close` (bez Brokera — zmniejsza uprawnienia).
    pub(crate) async fn close(&self, ctx: &ToolCtx) -> ToolOutcome {
        match self.sessions.remove(&ctx.holder.session) {
            Some(b) => {
                let (port, id) = (self.browser.clone(), b.id);
                let _ = blocking(move || port.close(id)).await;
                ToolOutcome::ok(
                    "Zamknięto przeglądarkę; zgody na hosty wygasły.",
                    serde_json::json!({"closed": true}),
                )
            }
            None => ToolOutcome::ok(
                "Przeglądarka nie była otwarta.",
                serde_json::json!({"closed": false}),
            ),
        }
    }
}
