//! Host operacji wtyczek (`PluginHost`) w aplikacji. Obrona w głąb: runtime już zapytał Brokera
//! za agentkę wywołującą, a host **sam** woła `Broker::verify(token, potrzebna zdolność, podmiot)`
//! przed każdą operacją (bez tokenu — odmowa; poza `log`). Dalej:
//! - `fs.read-text` — deny-lista Jądra, odczyt przez `FsPort`, limit rozmiaru, tylko UTF-8;
//! - `fs.write-text` — deny-lista, zapis przez dziennik cofania jako krok sesji (pre-image,
//!   „Cofnij”), nigdy z pominięciem dziennika;
//! - `net.get` — host z adresu ⊆ token (`verify`), deny-lista domen dostawców, klient
//!   [`crate::net::HttpsGet`] (tylko HTTPS, bez przekierowań, adresy publiczne).

use std::path::{Component, Path};
use std::sync::Arc;

use async_trait::async_trait;
use compliance_contract::{DenyChecker, DenyLists, PathEnv};
use platform_contract::FsPort;
use plugin_runtime_contract::{HostError, HostOp, PluginHost, ceilings};
use safety_broker_contract::{Broker, CapToken, Holder};
use serde_json::{Value, json};
use undo_journal_contract::{StepCtx, UndoJournal};

use crate::net::{HttpsGet, https_host};

/// Największy plik czytany przez wtyczkę i największa odpowiedź sieci (B) — sufit wejścia/wyjścia
/// wtyczki; wynik i tak przycina limit z manifestu.
pub const MAX_HOST_BYTES: usize = ceilings::IO_BYTES as usize;

/// Zależności hosta.
#[derive(Clone)]
pub struct HostDeps {
    /// Broker (weryfikacja tokenu przed każdą operacją).
    pub broker: Arc<dyn Broker>,
    /// Odczyt plików.
    pub fs: Arc<dyn FsPort>,
    /// Zapis plików z pre-image (cofanie).
    pub journal: Arc<dyn UndoJournal>,
    /// Środowisko ścieżek (profil właściciela).
    pub env: PathEnv,
    /// Deny-listy Jądra.
    pub deny: DenyLists,
    /// Klient sieci.
    pub net: Arc<dyn HttpsGet>,
}

/// Host operacji wtyczek.
pub struct AlfaPluginHost {
    broker: Arc<dyn Broker>,
    fs: Arc<dyn FsPort>,
    journal: Arc<dyn UndoJournal>,
    env: PathEnv,
    deny: DenyChecker,
    net: Arc<dyn HttpsGet>,
}

impl AlfaPluginHost {
    /// Host nad zależnościami.
    pub fn new(deps: HostDeps) -> Self {
        Self {
            deny: DenyChecker::new(deps.deny, &deps.env),
            broker: deps.broker,
            fs: deps.fs,
            journal: deps.journal,
            env: deps.env,
            net: deps.net,
        }
    }

    /// Ścieżka bezwzględna bez `.`/`..` (oba separatory) i spoza deny-listy Jądra.
    fn path<'a>(&self, raw: &'a str) -> Result<&'a Path, HostError> {
        let p = Path::new(raw);
        let dotted = raw.split(['\\', '/']).any(|seg| seg == ".." || seg == ".")
            || p.components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir));
        if dotted || raw.contains(['%', '~']) {
            return Err(HostError::BadArgs(
                "ścieżka musi być bezwzględna i jawna".into(),
            ));
        }
        if self.deny.is_denied_path(raw, &self.env) {
            return Err(HostError::Denied(
                "ścieżka na deny-liście Jądra (sekrety, poświadczenia)".into(),
            ));
        }
        Ok(p)
    }

    fn read(&self, raw: &str) -> Result<Value, HostError> {
        let path = self.path(raw)?;
        let bytes = self
            .fs
            .read(path)
            .map_err(|e| HostError::Failed(e.to_string()))?;
        if bytes.len() > MAX_HOST_BYTES {
            return Err(HostError::TooLarge(MAX_HOST_BYTES));
        }
        let content = String::from_utf8(bytes)
            .map_err(|_| HostError::Failed("plik nie jest tekstem UTF-8".into()))?;
        Ok(json!({ "content": content }))
    }

    fn write(&self, raw: &str, content: &str, holder: &Holder) -> Result<Value, HostError> {
        let path = self.path(raw)?;
        let fail = |e: undo_journal_contract::UndoError| HostError::Failed(e.to_string());
        let mut ctx = StepCtx::new(
            holder.session.as_str(),
            holder.agent.as_ref().map(|a| a.as_str()),
            "Zapis wtyczki",
        );
        ctx.label = format!("Wtyczka zapisała „{raw}”");
        let step = self.journal.begin_step(ctx).map_err(fail)?;
        if let Err(e) = self.journal.write(step, path, content.as_bytes()) {
            let _ = self.journal.abort_step(step);
            return Err(fail(e));
        }
        let summary = self.journal.commit_step(step).map_err(fail)?;
        Ok(json!({ "written": content.len(), "undo_step": summary.step.0 }))
    }

    async fn get(&self, url: &str) -> Result<Value, HostError> {
        let host = https_host(url).ok_or_else(|| {
            HostError::BadArgs("dozwolone tylko https:// do hostów publicznych".into())
        })?;
        if self.deny.is_denied_domain(&host) {
            return Err(HostError::Denied(format!(
                "host {host} na deny-liście Jądra (domeny dostawców)"
            )));
        }
        let r = self
            .net
            .get(url, MAX_HOST_BYTES)
            .await
            .map_err(HostError::Failed)?;
        Ok(json!({ "status": r.status, "content_type": r.content_type, "body": r.body }))
    }
}

#[async_trait]
impl PluginHost for AlfaPluginHost {
    async fn execute(
        &self,
        op: &HostOp,
        token: Option<&CapToken>,
        holder: &Holder,
    ) -> Result<Value, HostError> {
        let Some(needed) = op.capability()? else {
            return Ok(Value::Null);
        };
        let token = token.ok_or_else(|| HostError::Denied("operacja bez tokenu Brokera".into()))?;
        self.broker
            .verify(token, &needed, holder)
            .map_err(|e| HostError::Denied(e.to_string()))?;
        match op {
            HostOp::Log { .. } => Ok(Value::Null),
            HostOp::FsReadText { path } => self.read(path),
            HostOp::FsWriteText { path, content } => self.write(path, content, holder),
            HostOp::NetGet { url } => self.get(url).await,
        }
    }
}
