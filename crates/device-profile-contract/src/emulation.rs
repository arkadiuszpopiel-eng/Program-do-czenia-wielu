//! Metody profilu: główne GPU, bateria, emulacja baseline (PLAN §3.5 „Testowanie baseline”).

use crate::types::{
    BASELINE_LIMITS, Emulation, EmulationFactors, GpuInfo, GpuVendor, PowerState, Profile,
    ResourceLimits,
};

/// Próg VRAM (MB), od którego GPU gospodarza jest wyraźnie szybsze od RX 7600 (korekta ×2,2).
const FAST_GPU_VRAM_MB: u32 = 12_000;
/// L3 baseline (Ryzen 5 5600) w MB — większa pamięć podręczna zawyża wyniki CPU.
const BASELINE_L3_MB: u32 = 32;

impl Profile {
    /// Główne GPU: największy VRAM (przy remisie NVIDIA — CUDA).
    pub fn primary_gpu(&self) -> Option<&GpuInfo> {
        self.gpus.iter().max_by_key(|g| {
            (
                g.vram_mb,
                u8::from(g.vendor == GpuVendor::Nvidia),
                u8::from(!g.backends.is_empty()),
            )
        })
    }

    /// VRAM głównego GPU w MB (0 = brak GPU).
    pub fn primary_vram_mb(&self) -> u32 {
        self.primary_gpu().map_or(0, |g| g.vram_mb)
    }

    /// Czy maszyna pracuje na baterii.
    pub fn on_battery(&self) -> bool {
        matches!(self.power, PowerState::Battery { .. })
    }

    /// Korekta czasów dla tego gospodarza przy emulacji baseline: GPU ×2,2 tylko na GPU wyraźnie
    /// szybszym od RX 7600 (≥ 12 GB, np. RX 9070 XT), CPU +25% przy większym L3 lub > 12 wątkach.
    pub fn baseline_factors(&self) -> EmulationFactors {
        let fast_gpu = self.primary_vram_mb() >= FAST_GPU_VRAM_MB;
        let fast_cpu = self.cpu.l3_cache_mb.is_some_and(|l3| l3 > BASELINE_L3_MB)
            || self.cpu.logical_cores > BASELINE_LIMITS.cpu_threads;
        EmulationFactors {
            gpu_time_pct: if fast_gpu {
                EmulationFactors::DESKTOP_TO_BASELINE.gpu_time_pct
            } else {
                100
            },
            cpu_time_pct: if fast_cpu {
                EmulationFactors::DESKTOP_TO_BASELINE.cpu_time_pct
            } else {
                100
            },
        }
    }

    /// Profil z narzuconymi limitami (wartości tylko maleją) i zapisaną emulacją.
    pub fn emulate(&self, limits: ResourceLimits, factors: EmulationFactors) -> Profile {
        let mut out = self.clone();
        out.cpu.physical_cores = out.cpu.physical_cores.min(limits.cpu_cores);
        out.cpu.logical_cores = out
            .cpu
            .logical_cores
            .min(limits.cpu_threads)
            .max(out.cpu.physical_cores);
        out.ram_mb = out.ram_mb.min(limits.ram_mb);
        for gpu in &mut out.gpus {
            gpu.vram_mb = gpu.vram_mb.min(limits.vram_mb);
        }
        out.emulation = Some(Emulation { limits, factors });
        out
    }

    /// Emulacja baseline: 6 rdzeni / 12 wątków, 16 GB RAM, VRAM 8 GB + korekta czasów gospodarza.
    pub fn emulate_baseline(&self) -> Profile {
        self.emulate(BASELINE_LIMITS, self.baseline_factors())
    }
}
