//! `tools-media` — implementacja (docs/modules/tools-media/SPEC.md, PLAN §7.2 „Multimedia”).
//!
//! Każde wywołanie: ścieżka (deny-lista także po dowiązaniach — przed Brokerem) → Broker
//! `fs.read(plik)` (konwersja: także `fs.write(nowy plik)`; weryfikacja każdego tokenu) →
//! - `media_info`: nagłówki przez `lib-media` (odczyt fragmentami, budżet bajtów);
//! - `media_convert`: [`Transcoder`] (aplikacja: [`FfmpegTranscoder`] — sidecar w Job Object,
//!   lista zamknięta formatów, wymuszony demuxer, tylko protokół `file`) → **nowy** plik przez
//!   dziennik cofania (nigdy nadpisanie, krok „Cofnij” go usuwa);
//! - `media_play`: WAV (albo konwersja do WAV) → mono w częstotliwości z listy → [`AudioPlayer`]
//!   (aplikacja: [`SpeakerPlayer`] — kolejka mówienia, ducking i zatrzymanie przy wywłaszczeniu).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod convert;
mod core;
mod ffmpeg;
mod info;
mod play;
mod player;

use std::sync::Arc;

use async_trait::async_trait;
use compliance_contract::{DenyChecker, DenyLists, PathEnv};
use core_bus_contract::EventBus;
use core_registry_contract::{ManifestError, ModuleManifest};
use lib_media::RangeRead;
use platform_contract::FsPort;
use safety_broker_contract::Broker;
use tools_common_contract::{
    BrokerGate, Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, Toolset,
};
use tools_media_contract::{AudioPlayer, MediaToolsConfig, Transcoder, manifests};
use undo_journal_contract::UndoJournal;

use crate::core::Core;
pub use ffmpeg::{FFMPEG_TIMEOUT_MS, FfmpegConfig, FfmpegTranscoder, ffmpeg_args};
pub use player::{PlayerConfig, SpeakerPlayer};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Manifest modułu (rejestr).
pub fn module_manifest() -> Result<ModuleManifest, ManifestError> {
    ModuleManifest::parse_toml(MODULE_TOML)
}

/// Zależności narzędzi multimediów.
#[derive(Clone)]
pub struct MediaToolsDeps {
    /// System plików (istnienie nowego pliku).
    pub fs: Arc<dyn FsPort>,
    /// Odczyt plików fragmentami (nagłówki bez wczytywania całości).
    pub files: Arc<dyn RangeRead>,
    /// Dziennik cofania (zapis nowego pliku).
    pub journal: Arc<dyn UndoJournal>,
    /// Konwerter (ffmpeg).
    pub transcoder: Arc<dyn Transcoder>,
    /// Odtwarzacz (głośnik Alfy).
    pub player: Arc<dyn AudioPlayer>,
    /// Broker.
    pub broker: Arc<dyn Broker>,
    /// Środowisko ścieżek (profil właściciela).
    pub env: PathEnv,
    /// Deny-listy Jądra.
    pub deny: DenyLists,
    /// Limity.
    pub config: MediaToolsConfig,
    /// Magistrala (`tool.media.*`, bez treści).
    pub bus: Option<Arc<dyn EventBus>>,
}

/// Zestaw narzędzi multimediów.
#[derive(Clone)]
pub struct MediaTools {
    core: Arc<Core>,
}

impl MediaTools {
    /// Zestaw nad zależnościami.
    pub fn new(deps: MediaToolsDeps) -> Self {
        Self {
            core: Arc::new(Core {
                deny: DenyChecker::new(deps.deny, &deps.env),
                fs: deps.fs,
                files: deps.files,
                journal: deps.journal,
                transcoder: deps.transcoder,
                player: deps.player,
                gate: BrokerGate::new(deps.broker),
                env: deps.env,
                config: deps.config,
                bus: deps.bus,
            }),
        }
    }

    /// Kill-switch: zatrzymuje całe odtwarzanie; zwraca liczbę zatrzymanych klipów.
    pub fn stop_all(&self) -> usize {
        self.core.player.stop_all()
    }
}

impl Toolset for MediaTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        manifests()
            .into_iter()
            .map(|manifest| {
                Arc::new(MediaTool {
                    core: self.core.clone(),
                    manifest,
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}

struct MediaTool {
    core: Arc<Core>,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for MediaTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        if !args.is_object() {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                "Niepoprawne argumenty: oczekiwano obiektu JSON zgodnego ze schematem narzędzia.",
            );
        }
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        let (core, m) = (&self.core, &self.manifest);
        let r = match m.name.as_str() {
            "media_info" => core.info(args, ctx, m).await,
            "media_convert" => core.convert(args, ctx, m).await,
            _ => core.play(args, ctx, m).await,
        };
        r.unwrap_or_else(|out| *out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_manifest_parses() {
        assert_eq!(module_manifest().unwrap().id.as_str(), "tools-media");
    }
}
