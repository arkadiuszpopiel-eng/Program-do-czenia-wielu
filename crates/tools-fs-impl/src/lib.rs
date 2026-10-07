//! `tools-fs` — implementacja (docs/modules/tools-fs/SPEC.md, PLAN §7.2, §8.1, §8.7).
//!
//! Każde wywołanie: ścieżka od modelu → postać jednoznaczna (bez `..`, ADS, urządzeń) →
//! deny-lista (Jądro + poświadczenia platformy) **przed** Brokerem → `decide` (token albo
//! zatwierdzenie w Broker-UI z limitem czasu albo odmowa z powodem dla modelu) → `verify`
//! przy użyciu → `FsPort` (odczyt) albo dziennik cofania (mutacja: pre-image, brak wpisu =
//! brak operacji) → unieważnienie tokenu. Treść i nazwy plików to niezaufane dane: Broker
//! dostaje zgłoszenie taint, wynik ma oznaczenie dla runtime.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod core;
mod read;
mod write;

use std::sync::Arc;

use async_trait::async_trait;
use core_registry_contract::{ManifestError, ModuleManifest};
use tools_common_contract::{Tool, ToolCtx, ToolManifest, ToolOutcome, Toolset};
use tools_fs_contract::{FsToolKind, manifest};

pub use crate::core::FsToolsDeps;
pub use crate::write::MKDIR_MARKER;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu (rejestr).
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Zestaw narzędzi plikowych.
#[derive(Clone)]
pub struct FsTools {
    core: Arc<crate::core::Core>,
}

impl FsTools {
    /// Zestaw nad zależnościami.
    pub fn new(deps: FsToolsDeps) -> Self {
        Self {
            core: Arc::new(crate::core::Core::new(deps)),
        }
    }

    /// Jedno narzędzie.
    pub fn tool(&self, kind: FsToolKind) -> Arc<dyn Tool> {
        Arc::new(FsTool {
            core: self.core.clone(),
            kind,
            manifest: manifest(kind),
        })
    }
}

impl Toolset for FsTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        FsToolKind::ALL.into_iter().map(|k| self.tool(k)).collect()
    }
}

struct FsTool {
    core: Arc<crate::core::Core>,
    kind: FsToolKind,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for FsTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        let core = &self.core;
        let m = &self.manifest;
        let result = match self.kind {
            FsToolKind::List => core.list(args, ctx, m).await,
            FsToolKind::Read => core.read(args, ctx, m).await,
            FsToolKind::Stat => core.stat(args, ctx, m).await,
            FsToolKind::Search => core.search(args, ctx, m).await,
            FsToolKind::Write => core.write(args, ctx, m).await,
            FsToolKind::Move => core.move_path(args, ctx, m).await,
            FsToolKind::Copy => core.copy(args, ctx, m).await,
            FsToolKind::Rename => core.rename(args, ctx, m).await,
            FsToolKind::Delete => core.delete(args, ctx, m).await,
            FsToolKind::DeletePermanent => core.delete_permanent(args, ctx, m).await,
            FsToolKind::Mkdir => core.mkdir(args, ctx, m).await,
        };
        result.unwrap_or_else(|out| *out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        let m = module_manifest().unwrap();
        assert_eq!(m.id.as_str(), "tools-fs");
    }
}
