//! `tools-clipboard` — implementacja (docs/modules/tools-clipboard/SPEC.md, PLAN §7.2, §8.7).
//!
//! Odczyt: Broker (`gui.control(clipboard.exe)`) → `ClipboardPort::get` → tekst zredagowany
//! i obcięty, obraz jako base64 (z limitem), pliki bez ścieżek z deny-listy → zgłoszenie taint
//! (treść spoza Alfy). Zapis: Broker → zapamiętanie poprzedniej zawartości → `set` → krok
//! „Cofnij” w rejestrze schowka; cofnięcie wykrywa konflikt (schowek zmieniony później).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use compliance_contract::{DenyChecker, DenyLists, PathEnv};
use core_bus_contract::{EventBus, Level};
use core_registry_contract::{ManifestError, ModuleManifest};
use platform_contract::{ClipboardContent, ClipboardPort};
use safety_broker_contract::{Broker, TaintSource};
use tools_clipboard_contract::{
    ClipFormat, ClipboardToolsConfig, ClipboardUndo, ClipboardUndoError, EVENT_GET, EVENT_SET,
    PNG_SIGNATURE, ReadArgs, ReadOutput, WriteArgs, WriteOutput, clipboard_capability,
    read_manifest, write_manifest,
};
use tools_common_contract::{
    Authorization, BrokerGate, Tool, ToolCtx, ToolErrorKind, ToolImage, ToolManifest, ToolOutcome,
    Toolset, UndoRef, UndoService, action_request, base_facts, parse_args, paths, report_untrusted,
    text, tool_event,
};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu (rejestr).
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Zależności narzędzi schowka.
#[derive(Clone)]
pub struct ClipboardToolsDeps {
    /// Port schowka.
    pub clipboard: Arc<dyn ClipboardPort>,
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Środowisko ścieżek (deny-lista listy plików).
    pub env: PathEnv,
    /// Deny-listy Jądra.
    pub deny: DenyLists,
    /// Limity.
    pub config: ClipboardToolsConfig,
    /// Magistrala.
    pub bus: Option<Arc<dyn EventBus>>,
}

struct Core {
    clipboard: Arc<dyn ClipboardPort>,
    gate: BrokerGate,
    env: PathEnv,
    deny: DenyChecker,
    config: ClipboardToolsConfig,
    bus: Option<Arc<dyn EventBus>>,
    ledger: Mutex<Ledger>,
}

#[derive(Default)]
struct Ledger {
    next: u64,
    entries: VecDeque<(u64, ClipboardContent, ClipboardContent)>,
}

type Step<T> = Result<T, Box<ToolOutcome>>;

impl Core {
    fn ledger(&self) -> MutexGuard<'_, Ledger> {
        self.ledger.lock().unwrap_or_else(|p| p.into_inner())
    }

    async fn emit(&self, name: &str, payload: serde_json::Value, ctx: &ToolCtx) {
        if let Some(bus) = &self.bus {
            let _ = bus
                .publish(tool_event(name, Level::Info, payload, ctx))
                .await;
        }
    }

    async fn authorize(
        &self,
        ctx: &ToolCtx,
        m: &ToolManifest,
        action: &str,
    ) -> Step<Authorization> {
        let cap = clipboard_capability().map_err(|e| {
            Box::new(ToolOutcome::failed(
                ToolErrorKind::Internal,
                format!("Zdolność schowka: {e}."),
            ))
        })?;
        let auth = self
            .gate
            .authorize(action_request(ctx, cap.clone(), base_facts(m, ctx)), ctx)
            .await
            .map_err(|e| Box::new(e.into_outcome(action)))?;
        if let Err(e) = self.gate.verify(&auth, &cap, &ctx.holder) {
            self.gate.release(std::slice::from_ref(&auth)).await;
            return Err(Box::new(e.into_outcome(action)));
        }
        Ok(auth)
    }

    fn render(&self, content: ClipboardContent) -> (String, ReadOutput, Vec<ToolImage>) {
        let mut out = ReadOutput {
            format: ClipFormat::Empty,
            text: None,
            files: Vec::new(),
            image_bytes: None,
            truncated: false,
        };
        match content {
            ClipboardContent::Empty => ("Schowek jest pusty.".to_owned(), out, Vec::new()),
            ClipboardContent::Text(t) => {
                let (body, cut) =
                    text::truncate_chars(&text::redact_secrets(&t), self.config.read_max_chars);
                out.format = ClipFormat::Text;
                out.truncated = cut;
                out.text = Some(body.clone());
                (format!("Schowek (tekst):\n{body}"), out, Vec::new())
            }
            ClipboardContent::Files(files) => {
                out.format = ClipFormat::Files;
                out.files = files
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .filter(|p| {
                        !self.deny.is_denied_path(p, &self.env) && !paths::has_credential_segment(p)
                    })
                    .collect();
                (
                    format!("Schowek (pliki):\n{}", out.files.join("\n")),
                    out,
                    Vec::new(),
                )
            }
            ClipboardContent::ImagePng(png) => {
                out.format = ClipFormat::Image;
                out.image_bytes = Some(png.len() as u64);
                if png.len() > self.config.image_max_bytes {
                    out.truncated = true;
                    (
                        format!(
                            "Schowek zawiera obraz PNG ({} B) — za duży, pominięty.",
                            png.len()
                        ),
                        out,
                        Vec::new(),
                    )
                } else {
                    let image = ToolImage {
                        media_type: "image/png".into(),
                        data_base64: STANDARD.encode(&png),
                    };
                    (
                        format!("Schowek zawiera obraz PNG ({} B).", png.len()),
                        out,
                        vec![image],
                    )
                }
            }
        }
    }

    async fn read(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let _: ReadArgs = parse_args(args)?;
        let auth = self.authorize(ctx, m, "odczyt schowka").await?;
        let content = self.clipboard.get();
        self.gate.release(std::slice::from_ref(&auth)).await;
        let content = content.map_err(|e| {
            Box::new(ToolOutcome::failed(
                ToolErrorKind::Io,
                format!("Nie odczytano schowka: {e}."),
            ))
        })?;
        let (body, data, images) = self.render(content);
        report_untrusted(&self.gate, ctx, TaintSource::Screen).await;
        self.emit(EVENT_GET, serde_json::json!({ "format": data.format }), ctx)
            .await;
        let mut out = ToolOutcome::ok(body, serde_json::to_value(&data).unwrap_or_default())
            .untrusted(TaintSource::Screen);
        out.truncated = data.truncated;
        out.images = images;
        out.approval = auth.approval;
        Ok(out)
    }

    fn content_of(&self, a: WriteArgs) -> Step<(ClipboardContent, ClipFormat, u64)> {
        let invalid = |m: &str| {
            Box::new(ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                m.to_owned(),
            ))
        };
        match (a.text, a.image_png_base64) {
            (Some(t), None) => {
                let n = t.chars().count();
                if n > self.config.write_max_chars {
                    return Err(invalid("Tekst przekracza limit zapisu schowka."));
                }
                Ok((ClipboardContent::Text(t), ClipFormat::Text, n as u64))
            }
            (None, Some(b64)) => {
                let png = STANDARD
                    .decode(b64.trim())
                    .map_err(|_| invalid("Niepoprawne base64 obrazu."))?;
                if !png.starts_with(&PNG_SIGNATURE) || png.len() > self.config.image_max_bytes {
                    return Err(invalid("Obraz musi być plikiem PNG w limicie rozmiaru."));
                }
                let n = png.len() as u64;
                Ok((ClipboardContent::ImagePng(png), ClipFormat::Image, n))
            }
            _ => Err(invalid(
                "Podaj dokładnie jedno: `text` albo `image_png_base64`.",
            )),
        }
    }

    async fn write(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
        m: &ToolManifest,
    ) -> Step<ToolOutcome> {
        let a: WriteArgs = parse_args(args)?;
        let (content, format, size) = self.content_of(a)?;
        let auth = self.authorize(ctx, m, "zapis schowka").await?;
        let result = (|| {
            let previous = self.clipboard.get()?;
            self.clipboard.set(content.clone())?;
            Ok::<_, platform_contract::PlatformError>(previous)
        })();
        self.gate.release(std::slice::from_ref(&auth)).await;
        let previous = result.map_err(|e| {
            Box::new(ToolOutcome::failed(
                ToolErrorKind::Io,
                format!("Nie zapisano schowka: {e}."),
            ))
        })?;
        let id = {
            let mut ledger = self.ledger();
            ledger.next += 1;
            let id = ledger.next;
            ledger.entries.push_back((id, previous, content));
            while ledger.entries.len() > self.config.undo_depth.max(1) {
                ledger.entries.pop_front();
            }
            id
        };
        self.emit(
            EVENT_SET,
            serde_json::json!({ "format": format, "size": size }),
            ctx,
        )
        .await;
        let data = WriteOutput {
            format,
            size,
            undo_id: id,
        };
        let mut out = ToolOutcome::ok(
            "Wstawiłam zawartość do schowka. Krok można cofnąć.",
            serde_json::to_value(&data).unwrap_or_default(),
        );
        out.undo = Some(UndoRef {
            service: UndoService::Clipboard,
            id,
            text: "Przywróć poprzednią zawartość schowka".into(),
        });
        out.approval = auth.approval;
        Ok(out)
    }
}

/// Zestaw narzędzi schowka (i rejestr cofania zapisów).
#[derive(Clone)]
pub struct ClipboardTools {
    core: Arc<Core>,
}

impl ClipboardTools {
    /// Zestaw nad zależnościami.
    pub fn new(deps: ClipboardToolsDeps) -> Self {
        Self {
            core: Arc::new(Core {
                deny: DenyChecker::new(deps.deny, &deps.env),
                clipboard: deps.clipboard,
                gate: BrokerGate::new(deps.broker),
                env: deps.env,
                config: deps.config,
                bus: deps.bus,
                ledger: Mutex::new(Ledger::default()),
            }),
        }
    }
}

impl ClipboardUndo for ClipboardTools {
    fn undo(&self, id: u64) -> Result<(), ClipboardUndoError> {
        let mut ledger = self.core.ledger();
        let pos = ledger
            .entries
            .iter()
            .position(|(i, _, _)| *i == id)
            .ok_or(ClipboardUndoError::Unknown(id))?;
        let current = self
            .core
            .clipboard
            .get()
            .map_err(|e| ClipboardUndoError::Platform(e.to_string()))?;
        if current != ledger.entries[pos].2 {
            return Err(ClipboardUndoError::Conflict);
        }
        let (_, previous, _) = ledger
            .entries
            .remove(pos)
            .ok_or(ClipboardUndoError::Unknown(id))?;
        self.core
            .clipboard
            .set(previous)
            .map_err(|e| ClipboardUndoError::Platform(e.to_string()))
    }
}

impl Toolset for ClipboardTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(ClipTool {
                core: self.core.clone(),
                manifest: read_manifest(),
                write: false,
            }),
            Arc::new(ClipTool {
                core: self.core.clone(),
                manifest: write_manifest(),
                write: true,
            }),
        ]
    }
}

struct ClipTool {
    core: Arc<Core>,
    manifest: ToolManifest,
    write: bool,
}

#[async_trait]
impl Tool for ClipTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        let r = if self.write {
            self.core.write(args, ctx, &self.manifest).await
        } else {
            self.core.read(args, ctx, &self.manifest).await
        };
        r.unwrap_or_else(|out| *out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        assert_eq!(module_manifest().unwrap().id.as_str(), "tools-clipboard");
    }
}
