//! Konfiguracja `[providers.local]` i argumenty `llama-server` z profilu urządzenia.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use device_profile_contract::{Backend, LocalLlm, Recommendation};
use lib_openai_compat::Timeouts;
use providers_contract::ProviderPrivacy;

use crate::manifest::{GPU_OVERHEAD_MB, ModelEntry};

/// Najmniejszy kontekst przy automatycznym zmniejszaniu (rozmowa z promptem systemowym i kilkoma
/// turami mieści się z zapasem; mniej — ryzyko „za długiej rozmowy" po kilku wymianach).
pub const MIN_CTX: u32 = 4_096;

/// VRAM dzierżawy STT na GPU (`voice-stt-impl`: whisper large-v3-turbo Q5 z kontekstem CUDA).
pub const STT_VRAM_RESERVE_MB: u32 = 1_500;

/// Wybór backendu (`[providers.local.machine] backend`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendChoice {
    /// Z rekomendacji `device-profile` (Vulkan na AMD, CUDA na NVIDIA, CPU bez GPU/na baterii).
    Auto,
    /// Wymuszony backend.
    Fixed(Backend),
}

/// Warstwy na GPU (`gpu_layers`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuLayers {
    /// Wg budżetu VRAM (pełne odciążenie, gdy model się mieści; inaczej proporcjonalnie).
    Auto,
    /// Stała liczba.
    Fixed(u32),
}

/// Konfiguracja dostawcy lokalnego.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalConfig {
    /// Identyfikator dostawcy (Router: `local:<model>`).
    pub provider_id: String,
    /// Katalog modeli (`%LOCALAPPDATA%\Alfa\models`).
    pub models_dir: PathBuf,
    /// Pliki `llama-server` per backend (osobne kompilacje llama.cpp).
    pub server_bin: BTreeMap<BackendKey, PathBuf>,
    /// Kandydaci na plik serwera per backend, w kolejności: pierwszy istniejący wygrywa przy
    /// każdym starcie sidecara ([`LocalConfig::server`]) — silnik pobrany w Ustawieniach działa
    /// bez ponownego uruchomienia aplikacji. Brak istniejącego kandydata → `server_bin`.
    pub server_candidates: BTreeMap<BackendKey, Vec<PathBuf>>,
    /// Model domyślny.
    pub default_model: String,
    /// Kontekst (`-c`) — największy; na GPU zmniejszany do `min_ctx` ([`LocalConfig::ctx_for`]).
    pub ctx: u32,
    /// Najmniejszy kontekst przy zmniejszaniu na małej karcie (`min_ctx = ctx` — stały kontekst).
    pub min_ctx: u32,
    /// VRAM zostawiany obok LLM dla STT na GPU, gdy profil urządzenia przewiduje STT na karcie
    /// (whisper large-v3-turbo Q5 — tyle zgłasza dzierżawa `voice-stt`).
    pub stt_reserve_mb: u32,
    /// Backend.
    pub backend: BackendChoice,
    /// Warstwy GPU.
    pub gpu_layers: GpuLayers,
    /// Wątki CPU (`--threads`); brak = rdzenie fizyczne.
    pub threads: Option<u32>,
    /// Limit VRAM (MB) bez zarządcy rezydencji; brak = budżet z `device-profile`.
    pub max_vram_mb: Option<u32>,
    /// Zwolnienie po bezczynności.
    pub idle_unload: Duration,
    /// Limit startu (ładowanie modelu).
    pub startup_timeout: Duration,
    /// Maksymalna liczba restartów po awarii w oknie.
    pub max_restarts: u32,
    /// Okno liczenia restartów.
    pub restart_window: Duration,
    /// Limity czasu strumienia (connect/first-token/idle).
    pub timeouts: Timeouts,
    /// Profil prywatności (domyślnie `local`/`local`: dane nie opuszczają maszyny).
    pub privacy: ProviderPrivacy,
}

/// Klucz backendu (porządek dla mapy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BackendKey {
    /// Vulkan.
    Vulkan,
    /// CUDA.
    Cuda,
    /// CPU.
    Cpu,
}

impl BackendKey {
    /// Z backendu `device-profile`.
    pub fn of(b: Backend) -> Self {
        match b {
            Backend::Vulkan => Self::Vulkan,
            Backend::Cuda => Self::Cuda,
            Backend::Cpu => Self::Cpu,
        }
    }

    /// Nazwa (zdarzenia, logi).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vulkan => "vulkan",
            Self::Cuda => "cuda",
            Self::Cpu => "cpu",
        }
    }
}

impl LocalConfig {
    /// Konfiguracja domyślna dla katalogu modeli i jednego pliku serwera (wszystkie backendy).
    pub fn new(models_dir: impl Into<PathBuf>, server: impl Into<PathBuf>) -> Self {
        let server = server.into();
        Self {
            provider_id: "local".into(),
            models_dir: models_dir.into(),
            server_bin: [BackendKey::Vulkan, BackendKey::Cuda, BackendKey::Cpu]
                .into_iter()
                .map(|k| (k, server.clone()))
                .collect(),
            server_candidates: BTreeMap::new(),
            default_model: "bielik-4.5b-v3.0-instruct-q8_0".into(),
            ctx: 8_192,
            min_ctx: MIN_CTX,
            stt_reserve_mb: STT_VRAM_RESERVE_MB,
            backend: BackendChoice::Auto,
            gpu_layers: GpuLayers::Auto,
            threads: None,
            max_vram_mb: None,
            idle_unload: Duration::from_secs(600),
            startup_timeout: Duration::from_secs(60),
            max_restarts: 3,
            restart_window: Duration::from_secs(600),
            timeouts: Timeouts {
                connect: Duration::from_secs(2),
                first_token: Duration::from_secs(60),
                idle: Duration::from_secs(30),
            },
            privacy: ProviderPrivacy::new("local", "local"),
        }
    }

    /// Plik serwera dla backendu w chwili startu: pierwszy istniejący kandydat
    /// (`server_candidates`), inaczej `server_bin`. Kandydat inny niż pierwszy (kompilacja
    /// zastępcza) — ostrzeżenie w dzienniku.
    pub fn server(&self, backend: BackendKey) -> Option<PathBuf> {
        let candidates = self.server_candidates.get(&backend);
        let found = candidates.and_then(|c| c.iter().position(|p| p.is_file()));
        match (candidates, found) {
            (Some(c), Some(i)) => {
                if i > 0 {
                    tracing::warn!(
                        backend = backend.as_str(),
                        server = %c[i].display(),
                        "brak llama-server dla backendu — używam kompilacji zastępczej"
                    );
                }
                Some(c[i].clone())
            }
            _ => self.server_bin.get(&backend).cloned(),
        }
    }

    /// Kontekst modelu na GPU z budżetem `budget_mb`, obok rezerwy `reserve_mb` (STT na karcie):
    /// `ctx` (≤ kontekst modelu), a gdy model z KV cache i rezerwą się nie mieści — największy
    /// kontekst połowiony do `min_ctx`, przy którym **całość** się mieści (pełne odciążenie obok
    /// whisper CUDA). Gdy nie mieści się nawet przy `min_ctx` — pełny `ctx`: i tak będzie częściowe
    /// odciążenie, a KV jednej warstwy to ułamek jej wag (mniejszy kontekst dałby 1–2 warstwy).
    pub fn ctx_for(&self, entry: &ModelEntry, budget_mb: u32, reserve_mb: u32) -> u32 {
        let max = self.ctx.min(entry.ctx);
        let floor = self.min_ctx.min(max);
        let fits = |ctx: u32| entry.vram_need(ctx).saturating_add(reserve_mb) <= budget_mb;
        let mut ctx = max;
        while !fits(ctx) {
            if ctx <= floor {
                return max;
            }
            ctx = (ctx / 2).max(floor);
        }
        ctx
    }

    /// Backend dla rekomendacji: wybór jawny, a przy `Auto` — CPU, gdy rekomendacja wyłącza
    /// lokalny LLM (bateria, słaby sprzęt), inaczej backend LLM z rekomendacji.
    pub fn backend_for(&self, rec: &Recommendation) -> BackendKey {
        match self.backend {
            BackendChoice::Fixed(b) => BackendKey::of(b),
            BackendChoice::Auto if rec.local_llm == LocalLlm::None => BackendKey::Cpu,
            BackendChoice::Auto => BackendKey::of(rec.llm_backend),
        }
    }
}

/// Plan uruchomienia `llama-server`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    /// Plik serwera.
    pub program: PathBuf,
    /// Backend.
    pub backend: BackendKey,
    /// Warstwy na GPU (`-ngl`).
    pub gpu_layers: u32,
    /// Kontekst.
    pub ctx: u32,
    /// Wątki.
    pub threads: u32,
}

/// Warstwy GPU wg budżetu VRAM dla kontekstu `ctx` (wagi + narzut + KV cache): pełne
/// odciążenie, gdy model się mieści; inaczej tyle warstw, ile mieści budżet po stałym narzucie
/// ([`GPU_OVERHEAD_MB`]) — `ModelEntry::vram_for(warstwy, ctx)` ≤ budżet.
pub fn layers_for(entry: &ModelEntry, vram_budget_mb: u32, ctx: u32) -> u32 {
    let need = entry.vram_need(ctx);
    if vram_budget_mb >= need {
        return entry.layers;
    }
    let per_layers = need.saturating_sub(GPU_OVERHEAD_MB);
    let free = vram_budget_mb.saturating_sub(GPU_OVERHEAD_MB);
    if per_layers == 0 || free == 0 {
        return 0;
    }
    let share = u64::from(entry.layers) * u64::from(free) / u64::from(per_layers);
    u32::try_from(share).unwrap_or(0).min(entry.layers)
}

impl LaunchPlan {
    /// Argumenty: zawsze `127.0.0.1`, losowy port i klucz per uruchomienie (nigdy `0.0.0.0`).
    pub fn args(
        &self,
        entry: &ModelEntry,
        model_path: &std::path::Path,
        port: u16,
        api_key: &str,
    ) -> Vec<String> {
        let mut args = vec![
            "--host".to_owned(),
            "127.0.0.1".into(),
            "--port".into(),
            port.to_string(),
            "--api-key".into(),
            api_key.to_owned(),
            "-m".into(),
            model_path.to_string_lossy().into_owned(),
            "--alias".into(),
            entry.id.clone(),
            "-c".into(),
            self.ctx.to_string(),
            "-ngl".into(),
            self.gpu_layers.to_string(),
            "--threads".into(),
            self.threads.to_string(),
            "-np".into(),
            "1".into(),
        ];
        if entry.tools {
            args.push("--jinja".into());
        }
        args
    }
}
