//! Rdzeń narzędzi sieciowych: zgoda Brokera `net.egress(host)` dla każdego nowego hosta (także po
//! przekierowaniu), weryfikacja tokenu przy każdym żądaniu, przekierowania według reguł
//! `lib-netguard` (bez obniżenia do `http`, bez adresów niepublicznych, limit liczby), odczyt
//! treści z limitem rozmiaru, czasu i anulowaniem.

use std::path::PathBuf;
use std::sync::Arc;

use compliance_contract::{DenyChecker, PathEnv};
use core_bus_contract::{EventBus, Level};
use lib_netguard::{Target, redirect_target};
use platform_apps_contract::DownloadStore;
use safety_broker_contract::{Capability, DeclaredFacts, HostPattern};
use tokio::time::Instant;
use tools_common_contract::{
    Authorization, BrokerGate, DenialReason, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome,
    action_request, base_facts, tool_event,
};
use tools_net_contract::{
    BodyReader, EVENT_REDIRECT, HttpMethod, HttpPort, HttpRequest, HttpResponse, NetError,
    NetToolsConfig, SearchPort,
};

/// Wynik pośredni (`Err` = gotowy wynik dla modelu).
pub(crate) type Step<T> = Result<T, Box<ToolOutcome>>;

pub(crate) fn fail(kind: ToolErrorKind, text: String) -> Box<ToolOutcome> {
    Box::new(ToolOutcome::failed(kind, text))
}

pub(crate) fn refuse(reason: DenialReason, action: &str, why: &str) -> Box<ToolOutcome> {
    let mut o = ToolOutcome::denied(reason, action);
    o.text = format!("Odmowa: {action} — {why}. Nie ponawiaj tej samej akcji.");
    Box::new(o)
}

/// Błąd sieci → wynik dla modelu.
pub(crate) fn net_failure(e: &NetError, action: &str) -> Box<ToolOutcome> {
    match e {
        NetError::Blocked(m) => refuse(DenialReason::Policy, action, m),
        NetError::Timeout => fail(
            ToolErrorKind::Timeout,
            format!("Nie wykonano: {action} — przekroczony limit czasu."),
        ),
        NetError::Unsupported(_) => fail(
            ToolErrorKind::Unsupported,
            format!("Nie wykonano: {action} — {e}."),
        ),
        _ => fail(ToolErrorKind::Io, format!("Nie wykonano: {action} — {e}.")),
    }
}

/// Zgoda na host z weryfikacją.
pub(crate) struct HostGrant {
    pub(crate) host: String,
    pub(crate) cap: Capability,
    pub(crate) auth: Authorization,
}

/// Odpowiedź po przekierowaniach.
pub(crate) struct Opened {
    pub(crate) response: HttpResponse,
    pub(crate) final_url: String,
    pub(crate) redirects: Vec<String>,
    pub(crate) grants: Vec<HostGrant>,
}

pub(crate) struct Core {
    pub(crate) http: Arc<dyn HttpPort>,
    pub(crate) search: Option<Arc<dyn SearchPort>>,
    pub(crate) downloads: Arc<dyn DownloadStore>,
    pub(crate) quarantine_root: Option<PathBuf>,
    pub(crate) gate: BrokerGate,
    pub(crate) deny: Arc<DenyChecker>,
    pub(crate) env: PathEnv,
    pub(crate) config: NetToolsConfig,
    pub(crate) bus: Option<Arc<dyn EventBus>>,
}

impl Core {
    pub(crate) async fn emit(&self, name: &str, payload: serde_json::Value, ctx: &ToolCtx) {
        if let Some(bus) = &self.bus {
            let _ = bus
                .publish(tool_event(name, Level::Info, payload, ctx))
                .await;
        }
    }

    /// Zgoda na jedną zdolność z weryfikacją tokenu.
    pub(crate) async fn authorize(
        &self,
        ctx: &ToolCtx,
        cap: &Capability,
        facts: DeclaredFacts,
        action: &str,
    ) -> Step<Authorization> {
        let auth = self
            .gate
            .authorize(action_request(ctx, cap.clone(), facts), ctx)
            .await
            .map_err(|e| Box::new(e.into_outcome(action)))?;
        if let Err(e) = self.gate.verify(&auth, cap, &ctx.holder) {
            self.gate.release(std::slice::from_ref(&auth)).await;
            return Err(Box::new(e.into_outcome(action)));
        }
        Ok(auth)
    }

    /// `net.egress(host)` — deny-lista domen dostawców przed Brokerem.
    pub(crate) async fn egress(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        host: &str,
        action: &str,
    ) -> Step<HostGrant> {
        if self.deny.is_denied_domain(host) {
            return Err(Box::new(ToolOutcome::denied(
                DenialReason::DenyList,
                &format!("{action} ({host})"),
            )));
        }
        let pattern = HostPattern::parse(host)
            .map_err(|e| refuse(DenialReason::Policy, action, &format!("host {host}: {e}")))?;
        let cap = Capability::NetEgress(pattern);
        let auth = self
            .authorize(ctx, &cap, base_facts(m, ctx), action)
            .await?;
        Ok(HostGrant {
            host: host.to_owned(),
            cap,
            auth,
        })
    }

    pub(crate) async fn release(&self, grants: &[HostGrant]) {
        let auths: Vec<Authorization> = grants.iter().map(|g| g.auth.clone()).collect();
        self.gate.release(&auths).await;
    }

    async fn send(
        &self,
        ctx: &ToolCtx,
        req: &HttpRequest,
        deadline: Instant,
        action: &str,
    ) -> Step<HttpResponse> {
        tokio::select! {
            () = ctx.cancel.cancelled() => Err(Box::new(ToolOutcome::cancelled(action))),
            r = tokio::time::timeout_at(deadline, self.http.send(req)) => match r {
                Ok(Ok(resp)) => Ok(resp),
                Ok(Err(e)) => Err(net_failure(&e, action)),
                Err(_) => Err(net_failure(&NetError::Timeout, action)),
            },
        }
    }

    /// Żądanie z przekierowaniami: nowy host = nowa zgoda Brokera; ten sam host — ta sama zgoda,
    /// zweryfikowana ponownie przed każdym żądaniem.
    pub(crate) async fn open(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        start: Target,
        method: HttpMethod,
        deadline: Instant,
        action: &str,
    ) -> Step<Opened> {
        let mut grants: Vec<HostGrant> = Vec::new();
        let r = self
            .follow(ctx, m, start, method, deadline, action, &mut grants)
            .await;
        match r {
            Ok((response, final_url, redirects)) => Ok(Opened {
                response,
                final_url,
                redirects,
                grants,
            }),
            Err(e) => {
                self.release(&grants).await;
                Err(e)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn follow(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        mut target: Target,
        method: HttpMethod,
        deadline: Instant,
        action: &str,
        grants: &mut Vec<HostGrant>,
    ) -> Step<(HttpResponse, String, Vec<String>)> {
        let mut redirects: Vec<String> = Vec::new();
        loop {
            let host = target.host().to_owned();
            let idx = match grants.iter().position(|g| g.host == host) {
                Some(i) => i,
                None => {
                    grants.push(self.egress(ctx, m, &host, action).await?);
                    grants.len() - 1
                }
            };
            let g = &grants[idx];
            self.gate
                .verify(&g.auth, &g.cap, &ctx.holder)
                .map_err(|e| Box::new(e.into_outcome(action)))?;
            let req = HttpRequest {
                url: target.as_str().to_owned(),
                method,
            };
            let resp = self.send(ctx, &req, deadline, action).await?;
            let Some(location) = resp.redirect_location() else {
                return Ok((resp, target.as_str().to_owned(), redirects));
            };
            if redirects.len() >= self.config.max_redirects as usize {
                return Err(refuse(
                    DenialReason::Policy,
                    action,
                    &format!("więcej niż {} przekierowań", self.config.max_redirects),
                ));
            }
            let next = redirect_target(target.url(), location).map_err(|e| {
                refuse(
                    DenialReason::Policy,
                    action,
                    &format!("przekierowanie na niedozwolony adres ({e})"),
                )
            })?;
            if next.host() != target.host() {
                self.emit(
                    EVENT_REDIRECT,
                    serde_json::json!({"from": target.host(), "to": next.host()}),
                    ctx,
                )
                .await;
            }
            redirects.push(next.as_str().to_owned());
            target = next;
        }
    }

    /// Następny fragment treści z limitem czasu i anulowaniem (`None` = koniec).
    pub(crate) async fn next_chunk(
        &self,
        ctx: &ToolCtx,
        body: &mut Box<dyn BodyReader>,
        deadline: Instant,
        action: &str,
    ) -> Step<Option<Vec<u8>>> {
        let next = tokio::select! {
            () = ctx.cancel.cancelled() => return Err(Box::new(ToolOutcome::cancelled(action))),
            r = tokio::time::timeout_at(deadline, body.chunk()) => r,
        };
        match next {
            Err(_) => Err(net_failure(&NetError::Timeout, action)),
            Ok(Err(e)) => Err(net_failure(&e, action)),
            Ok(Ok(c)) => Ok(c),
        }
    }

    /// Czyta treść do `limit` bajtów; zwraca (treść, obcięto). Nadmiar nie jest czytany.
    pub(crate) async fn read_body(
        &self,
        ctx: &ToolCtx,
        body: &mut Box<dyn BodyReader>,
        limit: u64,
        deadline: Instant,
        action: &str,
    ) -> Step<(Vec<u8>, bool)> {
        let mut out: Vec<u8> = Vec::new();
        while let Some(chunk) = self.next_chunk(ctx, body, deadline, action).await? {
            let room = usize::try_from(limit)
                .unwrap_or(usize::MAX)
                .saturating_sub(out.len());
            if chunk.len() > room {
                out.extend_from_slice(&chunk[..room]);
                return Ok((out, true));
            }
            out.extend_from_slice(&chunk);
        }
        Ok((out, false))
    }
}
