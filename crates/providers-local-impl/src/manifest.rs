//! Manifest modeli lokalnych (`models.toml`): GGUF z URL, rozmiarem, SHA-256, kwantyzacją
//! i szacunkami pamięci. Walidacja: tylko `.gguf`, **bez kwantów IQ** (PLAN §1.2, ADR 0014).

use providers_contract::{ModelCapabilities, ModelKind};
use serde::Deserialize;

use crate::error::LocalError;

/// Wbudowany manifest.
pub const MODELS_TOML: &str = include_str!("../models.toml");

/// Obsługiwana wersja formatu manifestu.
pub const MANIFEST_VERSION: u32 = 1;

/// Stały narzut VRAM niezależny od liczby warstw na GPU (kontekst CUDA/Vulkan ≈ 300 MB + bufory
/// obliczeń ≈ 250 MB przy ubatch 512) — część `vram_mb`; reszta rośnie z liczbą warstw.
pub const GPU_OVERHEAD_MB: u32 = 550;

/// RAM procesu `llama-server` poza wagami i KV cache warstw na CPU.
pub const PROCESS_RAM_MB: u32 = 512;

/// Wpis modelu.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    /// Identyfikator (model w `ChatRequest`, `--alias` serwera).
    pub id: String,
    /// Nazwa wyświetlana.
    pub name: String,
    /// Adres pobrania (https; testy: `http://127.0.0.1`).
    pub url: String,
    /// Nazwa pliku w katalogu modeli.
    pub file: String,
    /// Przybliżony rozmiar (MB) — do komunikatu w onboardingu; dokładny z `Content-Length`.
    pub size_mb: u32,
    /// SHA-256 (hex); pusty = zaufanie przy pierwszym pobraniu (ostrzeżenie w logach).
    #[serde(default)]
    pub sha256: String,
    /// Parametry (mld).
    pub params_b: f32,
    /// Kwantyzacja (np. `Q8_0`, `Q4_K_M`); IQ zabronione.
    pub quant: String,
    /// Liczba warstw (częściowe odciążenie GPU `-ngl`).
    pub layers: u32,
    /// Maksymalny kontekst modelu.
    pub ctx: u32,
    /// Szacunek VRAM przy pełnym odciążeniu **bez KV cache** (MB): wagi na GPU + stały narzut
    /// (kontekst CUDA/Vulkan, bufory obliczeń). KV dolicza [`ModelEntry::vram_need`].
    pub vram_mb: u32,
    /// Szacunek RAM przy wykonaniu na CPU **bez KV cache** (MB); KV — [`ModelEntry::ram_need`].
    pub ram_mb: u32,
    /// KV cache f16 na 1024 tokeny kontekstu (MB): warstwy × głowice KV × wymiar głowy × 2 (K, V)
    /// × 2 B × 1024 / 2²⁰. Rośnie z `-c`, więc liczony dla kontekstu uruchomienia.
    pub kv_mb_per_1k_ctx: u32,
    /// Czy szablon czatu obsługuje narzędzia (`--jinja`).
    #[serde(default)]
    pub tools: bool,
    /// Licencja modelu.
    pub license: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    schema_version: u32,
    #[serde(default)]
    model: Vec<ModelEntry>,
}

/// Czy nazwa kwantyzacji/pliku wskazuje kwant IQ (np. `IQ4_XS`, `iq3_m`).
pub fn is_iq_quant(text: &str) -> bool {
    text.to_ascii_uppercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|part| {
            part.strip_prefix("IQ")
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        })
}

/// Czy tekst to 64 znaki hex (albo pusty).
pub fn valid_sha256(text: &str) -> bool {
    text.is_empty() || (text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit()))
}

impl ModelEntry {
    /// Walidacja wpisu.
    pub fn validate(&self) -> Result<(), LocalError> {
        let bad = |m: String| Err(LocalError::Manifest(format!("`{}`: {m}", self.id)));
        if self.id.trim().is_empty() || self.id.contains(char::is_whitespace) {
            return bad("niepoprawny identyfikator".into());
        }
        if !self.file.to_ascii_lowercase().ends_with(".gguf")
            || self.file.contains(['/', '\\'])
            || self.file.contains("..")
        {
            return bad(format!("plik `{}` musi być nazwą pliku .gguf", self.file));
        }
        if is_iq_quant(&self.quant) || is_iq_quant(&self.file) {
            return bad("kwanty IQ są zabronione (crashe na RDNA3/Vulkan)".into());
        }
        if !valid_sha256(&self.sha256) {
            return bad("sha256 musi mieć 64 znaki hex albo być pusty".into());
        }
        let secure = self.url.starts_with("https://")
            || self.url.starts_with("http://127.0.0.1:")
            || self.url.starts_with("http://localhost:");
        if !secure {
            return bad("adres pobrania musi być https".into());
        }
        if self.layers == 0 || self.ctx == 0 || self.kv_mb_per_1k_ctx == 0 {
            return bad("warstwy, kontekst i KV cache muszą być > 0".into());
        }
        Ok(())
    }

    /// KV cache (MB, zaokrąglone w górę) dla kontekstu `ctx`.
    pub fn kv_mb(&self, ctx: u32) -> u32 {
        let mb = (u64::from(self.kv_mb_per_1k_ctx) * u64::from(ctx)).div_ceil(1024);
        u32::try_from(mb).unwrap_or(u32::MAX)
    }

    /// VRAM przy pełnym odciążeniu dla kontekstu `ctx`: wagi + narzut + KV cache.
    pub fn vram_need(&self, ctx: u32) -> u32 {
        self.vram_mb.saturating_add(self.kv_mb(ctx))
    }

    /// RAM przy wykonaniu na CPU dla kontekstu `ctx` (KV cache w RAM).
    pub fn ram_need(&self, ctx: u32) -> u32 {
        self.ram_mb.saturating_add(self.kv_mb(ctx))
    }

    /// VRAM przy `layers` warstwach na GPU (`-ngl`): stały narzut + proporcjonalna część wag
    /// i KV cache (KV warstwy leży tam, gdzie warstwa).
    pub fn vram_for(&self, layers: u32, ctx: u32) -> u32 {
        let need = self.vram_need(ctx);
        if layers == 0 {
            return 0;
        }
        if layers >= self.layers {
            return need;
        }
        let per_layers = u64::from(need.saturating_sub(GPU_OVERHEAD_MB)) * u64::from(layers)
            / u64::from(self.layers.max(1));
        let part = u32::try_from(per_layers).unwrap_or(u32::MAX);
        GPU_OVERHEAD_MB.min(need).saturating_add(part)
    }

    /// RAM procesu przy `layers` warstwach na GPU: proces + wagi i KV cache warstw na CPU.
    pub fn ram_for(&self, layers: u32, ctx: u32) -> u32 {
        let cpu_layers = self.layers.saturating_sub(layers);
        let part =
            u64::from(self.ram_need(ctx)) * u64::from(cpu_layers) / u64::from(self.layers.max(1));
        PROCESS_RAM_MB.saturating_add(u32::try_from(part).unwrap_or(u32::MAX))
    }

    /// Możliwości modelu dla Routera.
    pub fn capabilities(&self, ctx: u32) -> ModelCapabilities {
        ModelCapabilities {
            kinds: vec![ModelKind::Chat],
            context_window: Some(ctx.min(self.ctx)),
            max_output_tokens: Some(ctx.min(self.ctx) / 2),
            tools: self.tools,
            sampling: true,
            streaming: true,
            ..ModelCapabilities::default()
        }
    }
}

/// Parsuje i waliduje manifest.
pub fn parse_manifest(text: &str) -> Result<Vec<ModelEntry>, LocalError> {
    let file: ManifestFile =
        toml::from_str(text).map_err(|e| LocalError::Manifest(e.to_string()))?;
    if file.schema_version != MANIFEST_VERSION {
        return Err(LocalError::Manifest(format!(
            "nieobsługiwana wersja manifestu {}",
            file.schema_version
        )));
    }
    let mut ids = std::collections::BTreeSet::new();
    for m in &file.model {
        m.validate()?;
        if !ids.insert(m.id.clone()) {
            return Err(LocalError::Manifest(format!(
                "zduplikowany model `{}`",
                m.id
            )));
        }
    }
    Ok(file.model)
}

/// Wbudowany manifest (zwalidowany testem).
pub fn builtin_models() -> Result<Vec<ModelEntry>, LocalError> {
    parse_manifest(MODELS_TOML)
}
