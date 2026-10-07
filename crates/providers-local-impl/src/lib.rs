//! Lokalny `ModelProvider`: llama.cpp jako sidecar `llama-server` (docs/modules/providers-local/SPEC.md,
//! ADR 0014, PLAN §1.2, §5.2).
//!
//! - [`LocalProvider`] — strumień przez wspólny silnik `lib-openai-compat` (Chat Completions na
//!   `127.0.0.1`, losowy port i klucz API per uruchomienie; nigdy `0.0.0.0`), koszt 0;
//! - [`Sidecar`] — start na żądanie, `/health`, restart po awarii z limitem, fallback GPU → CPU,
//!   dzierżawa `model-residency`, zwolnienie po bezczynności;
//! - argumenty z profilu urządzenia ([`LocalConfig`], [`LaunchPlan`]): backend Vulkan/CUDA/CPU,
//!   `-ngl` wg budżetu VRAM, `-c`, `--threads`;
//! - [`Downloader`] — pobieranie GGUF z wznawianiem (HTTP Range) i SHA-256; manifest
//!   [`MODELS_TOML`] (Bielik 4.5B i 1.5B v3.0 Q8_0 z oficjalnych repozytoriów), bez kwantów IQ;
//! - rozliczenie pamięci z KV cache i częściowym odciążeniem ([`ModelEntry::vram_for`]);
//! - okno rozmowy ([`fit`]) — do serwera najnowsze tury mieszczące się w kontekście uruchomienia;
//! - [`LocalModule`] — moduł rejestru, zdarzenia `local.*`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod codec;
mod config;
mod download;
mod error;
mod manifest;
mod module;
mod placement;
mod process;
mod provider;
mod sidecar;
mod window;

pub use codec::LlamaCodec;
pub use config::{
    BackendChoice, BackendKey, GpuLayers, LaunchPlan, LocalConfig, MIN_CTX, STT_VRAM_RESERVE_MB,
    layers_for,
};
pub use download::{Downloaded, Downloader, MAX_RESUMES, hash_path, installed, part_path};
pub use error::{
    DownloadProgress, EVENT_BACKEND_FALLBACK, EVENT_DOWNLOAD_FAILED, EVENT_DOWNLOAD_FINISHED,
    EVENT_DOWNLOAD_PROGRESS, EVENT_LOADED, EVENT_SIDECAR_CRASHED, EVENT_UNLOADED, LocalError,
    LocalEvent,
};
pub use manifest::{
    GPU_OVERHEAD_MB, MANIFEST_VERSION, MODELS_TOML, ModelEntry, PROCESS_RAM_MB, builtin_models,
    is_iq_quant, parse_manifest, valid_sha256,
};
pub use module::{LocalModule, MODULE_TOML};
pub use process::{LaunchSpec, SidecarLauncher, SidecarProcess, TokioLauncher};
pub use provider::LocalProvider;
pub use sidecar::{RESIDENCY_OWNER, Sidecar};
pub use window::{Fitted, WINDOW_MARGIN, estimate_tokens, fit, reply_reserve};
