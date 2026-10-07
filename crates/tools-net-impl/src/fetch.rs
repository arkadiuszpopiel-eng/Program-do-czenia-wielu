//! `net_fetch` (GET/HEAD z limitem treści, tekst tylko dla typów tekstowych, niezaufany)
//! i `net_search` (dostawca przez port; bez dostawcy — „nieobsługiwane”).

use std::time::Duration;

use safety_broker_contract::TaintSource;
use tokio::time::Instant;
use tools_common_contract::{
    DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, parse_args, report_untrusted,
    text,
};
use tools_net_contract::{
    EVENT_FETCH, FetchArgs, FetchOut, HttpMethod, SearchArgs, SearchHit, SearchOut,
    url_carries_secret,
};

use crate::core::{Core, Step, fail, net_failure, refuse};

/// Czy typ treści jest tekstowy (brak typu — rozstrzyga dekodowanie).
fn textual(content_type: Option<&str>) -> bool {
    content_type.is_none_or(|ct| {
        let ct = ct.to_ascii_lowercase();
        ct.starts_with("text/")
            || [
                "json",
                "xml",
                "javascript",
                "csv",
                "yaml",
                "x-www-form-urlencoded",
            ]
            .iter()
            .any(|t| ct.contains(t))
    })
}

/// Adres od modelu: reguły `lib-netguard` i brak sekretów w adresie (eksfiltracja przez URL).
pub(crate) fn checked_target(url: &str, action: &str) -> Step<lib_netguard::Target> {
    let target = lib_netguard::check_url(url)
        .map_err(|e| refuse(DenialReason::Policy, action, &e.to_string()))?;
    if url_carries_secret(url) {
        return Err(refuse(
            DenialReason::Policy,
            action,
            "adres wygląda na zawierający sekret (klucz, token, hasło) — nie wysyłam go",
        ));
    }
    Ok(target)
}

impl Core {
    /// `net_fetch`.
    pub(crate) async fn fetch(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: FetchArgs = parse_args(args)?;
        let method = a.method.unwrap_or(HttpMethod::Get);
        let action = format!("pobranie {}", a.url);
        let target = checked_target(&a.url, &action)?;
        let deadline = Instant::now() + Duration::from_millis(self.config.fetch_timeout_ms);
        let mut opened = self
            .open(ctx, m, target.clone(), method, deadline, &action)
            .await?;
        let read = if method == HttpMethod::Head {
            Ok((Vec::new(), false))
        } else {
            self.read_body(
                ctx,
                &mut opened.response.body,
                self.config.max_fetch_bytes,
                deadline,
                &action,
            )
            .await
        };
        self.release(&opened.grants).await;
        let (body, cut) = read?;
        let bytes = body.len() as u64;
        report_untrusted(&self.gate, ctx, TaintSource::Web).await;
        let resp = &opened.response;
        let max_chars = a.max_chars.unwrap_or(self.config.max_chars) as usize;
        let (page, chars_cut) = match (textual(resp.content_type.as_deref()), method) {
            (true, HttpMethod::Get) => match text::decode_text(&body) {
                Some(t) => {
                    let (t, c) = text::truncate_chars(&text::redact_secrets(&t), max_chars);
                    (Some(t), c)
                }
                None => (None, false),
            },
            _ => (None, false),
        };
        let out = FetchOut {
            url: target.as_str().to_owned(),
            final_url: opened.final_url.clone(),
            status: resp.status,
            content_type: resp.content_type.clone(),
            bytes,
            text: page,
            truncated: cut || chars_cut,
            redirects: opened.redirects.clone(),
        };
        let host = lib_netguard::check_url(&out.final_url)
            .map(|t| t.host().to_owned())
            .unwrap_or_default();
        self.emit(
            EVENT_FETCH,
            serde_json::json!({"host": host, "status": out.status, "bytes": out.bytes}),
            ctx,
        )
        .await;
        let summary = format!(
            "HTTP {} {} ({}, {} B{}{})",
            out.status,
            out.final_url,
            out.content_type.as_deref().unwrap_or("typ nieznany"),
            out.bytes,
            if out.truncated { ", obcięto" } else { "" },
            if out.redirects.is_empty() {
                String::new()
            } else {
                format!(", przekierowania: {}", out.redirects.len())
            }
        );
        let body_text = out.text.clone().unwrap_or_else(|| {
            if method == HttpMethod::Head {
                String::new()
            } else {
                "[treść nietekstowa — użyj `net_download`]".into()
            }
        });
        let (rendered, _) = text::truncate_chars(
            &format!("{summary}\n{body_text}"),
            self.config.output_max_chars,
        );
        let mut o = ToolOutcome::ok(rendered, serde_json::to_value(&out).unwrap_or_default())
            .untrusted(TaintSource::Web);
        o.truncated = out.truncated;
        o.approval = opened.grants.iter().find_map(|g| g.auth.approval);
        Ok(o)
    }

    /// `net_search`.
    pub(crate) async fn search(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: SearchArgs = parse_args(args)?;
        let action = "wyszukiwanie w sieci";
        let Some(port) = self.search.clone() else {
            return Err(fail(
                ToolErrorKind::Unsupported,
                format!("Nie wykonano: {action} — brak dostawcy wyszukiwania."),
            ));
        };
        let Some(host) = port.endpoint_host() else {
            return Err(fail(
                ToolErrorKind::Unsupported,
                format!("Nie wykonano: {action} — brak dostawcy wyszukiwania."),
            ));
        };
        let target = checked_target(&format!("https://{host}/"), action)?;
        let deadline = Instant::now() + Duration::from_millis(self.config.fetch_timeout_ms);
        let grant = self.egress(ctx, m, target.host(), action).await?;
        let max = a.max_results.unwrap_or(10).clamp(1, 20);
        let found = tokio::select! {
            () = ctx.cancel.cancelled() => Err(Box::new(ToolOutcome::cancelled(action))),
            r = tokio::time::timeout_at(deadline, port.search(&a.query, max)) => match r {
                Ok(Ok(hits)) => Ok(hits),
                Ok(Err(e)) => Err(net_failure(&e, action)),
                Err(_) => Err(net_failure(&tools_net_contract::NetError::Timeout, action)),
            },
        };
        self.release(std::slice::from_ref(&grant)).await;
        let hits = found?;
        report_untrusted(&self.gate, ctx, TaintSource::Web).await;
        let out = SearchOut {
            results: hits
                .into_iter()
                .map(|h| SearchHit {
                    title: text::redact_secrets(&h.title),
                    url: h.url,
                    snippet: text::redact_secrets(&h.snippet),
                })
                .collect(),
        };
        let lines: Vec<String> = out
            .results
            .iter()
            .map(|h| format!("- {} — {}\n  {}", h.title, h.url, h.snippet))
            .collect();
        let (rendered, _) = text::truncate_chars(
            &format!("Wyniki ({}):\n{}", out.results.len(), lines.join("\n")),
            self.config.output_max_chars,
        );
        let mut o = ToolOutcome::ok(rendered, serde_json::to_value(&out).unwrap_or_default())
            .untrusted(TaintSource::Web);
        o.approval = grant.auth.approval;
        Ok(o)
    }
}
