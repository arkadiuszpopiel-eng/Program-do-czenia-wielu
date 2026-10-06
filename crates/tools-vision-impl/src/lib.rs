//! `tools-vision` — implementacja (docs/modules/tools-vision/SPEC.md, PLAN §7.1–7.2; S26).
//!
//! Obraz: zrzut przez `ScreenCapturePort` (maskowanie okien Alfy/Brokera, aplikacji z deny-listy
//! i dostawców planów, pól haseł — w porcie, przed OCR i przed modelem; okno chronione = odmowa)
//! po zgodzie Brokera `gui.control(...)`, albo plik po deny-liście ścieżek i zgodzie `fs.read(plik)`,
//! z wymiarami z nagłówka (`lib-media`) przed dekodowaniem. Następnie:
//! - `vision_ocr` → `OcrPort` na wątku blokującym → linie ze współrzędnymi ekranu;
//! - `vision_describe` → `DescribePort` z prywatnością sesji ([`RouterDescriber`]: Router
//!   hybrydowy dla zwykłej sesji, wyłącznie lokalny dla prywatnej/„tylko lokalnie”).
//!
//! Wynik to treść niezaufana (taint zgłoszony Brokerowi), sekrety redagowane; piksele i tekst
//! nigdy nie trafiają do zdarzeń ani na dysk.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod describe;
mod image;
mod ocr;
mod router;

use std::sync::Arc;

use async_trait::async_trait;
use compliance_contract::{DenyChecker, DenyLists, PathEnv};
use core_bus_contract::EventBus;
use core_registry_contract::{ManifestError, ModuleManifest};
use lib_media::RangeRead;
use platform_contract::{DesktopPort, ScreenCapturePort};
use safety_broker_contract::Broker;
use tools_common_contract::{BrokerGate, Tool, ToolCtx, ToolManifest, ToolOutcome, Toolset};
use tools_vision_contract::{DescribePort, OcrPort, PrivacyLookup, VisionToolsConfig, manifests};
use tools_window_contract::gui;

pub use router::RouterDescriber;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu (rejestr).
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Zależności narzędzi wizji.
#[derive(Clone)]
pub struct VisionToolsDeps {
    /// Okna (cel zrzutu, strażnik okien Alfy/Brokera).
    pub desktop: Arc<dyn DesktopPort>,
    /// Zrzuty z maskowaniem.
    pub capture: Arc<dyn ScreenCapturePort>,
    /// OCR.
    pub ocr: Arc<dyn OcrPort>,
    /// Opis obrazu (Router dla klasy „GUI/wizja”).
    pub describer: Arc<dyn DescribePort>,
    /// Tag prywatności sesji.
    pub privacy: Arc<dyn PrivacyLookup>,
    /// Odczyt plików obrazów (fragmentami — nagłówek przed całością).
    pub files: Arc<dyn RangeRead>,
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Środowisko ścieżek (profil właściciela).
    pub env: PathEnv,
    /// Deny-listy Jądra.
    pub deny: DenyLists,
    /// Limity.
    pub config: VisionToolsConfig,
    /// Magistrala (`tool.vision.*`, bez treści).
    pub bus: Option<Arc<dyn EventBus>>,
}

/// Rdzeń współdzielony przez narzędzia.
pub(crate) struct Core {
    pub(crate) desktop: Arc<dyn DesktopPort>,
    pub(crate) capture: Arc<dyn ScreenCapturePort>,
    pub(crate) ocr: Arc<dyn OcrPort>,
    pub(crate) describer: Arc<dyn DescribePort>,
    pub(crate) privacy: Arc<dyn PrivacyLookup>,
    pub(crate) files: Arc<dyn RangeRead>,
    pub(crate) gate: BrokerGate,
    pub(crate) env: PathEnv,
    pub(crate) deny: DenyChecker,
    pub(crate) config: VisionToolsConfig,
    pub(crate) bus: Option<Arc<dyn EventBus>>,
}

/// Zestaw narzędzi wizji.
#[derive(Clone)]
pub struct VisionTools {
    core: Arc<Core>,
}

impl VisionTools {
    /// Zestaw nad zależnościami.
    pub fn new(deps: VisionToolsDeps) -> Self {
        Self {
            core: Arc::new(Core {
                deny: DenyChecker::new(deps.deny, &deps.env),
                desktop: deps.desktop,
                capture: deps.capture,
                ocr: deps.ocr,
                describer: deps.describer,
                privacy: deps.privacy,
                files: deps.files,
                gate: BrokerGate::new(deps.broker),
                env: deps.env,
                config: deps.config,
                bus: deps.bus,
            }),
        }
    }
}

impl Toolset for VisionTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        manifests()
            .into_iter()
            .map(|manifest| {
                Arc::new(VisionTool {
                    core: self.core.clone(),
                    manifest,
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}

struct VisionTool {
    core: Arc<Core>,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for VisionTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        if let Err(out) = gui::object_args(&args) {
            return *out;
        }
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        let (core, m) = (&self.core, &self.manifest);
        let r = match m.name.as_str() {
            "vision_ocr" => core.ocr(args, ctx, m).await,
            _ => core.describe(args, ctx, m).await,
        };
        r.unwrap_or_else(|out| *out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        assert_eq!(module_manifest().unwrap().id.as_str(), "tools-vision");
    }
}
