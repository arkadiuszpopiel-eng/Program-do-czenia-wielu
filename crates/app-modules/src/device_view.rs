//! Profil sprzętu w kształcie UI (komendy `device_*`): profil i rekomendacja z `device-profile`
//! oraz odczyt kart na żywo z `gpu_mem` → DTO. Czyste przekształcenia (przeniesione z `app-core`).

use app_api::dto::{
    self, BatteryView, CpuView, DeviceProfile, GpuLive, GpuLiveCard, GpuLiveProcess, GpuView,
    HwClass, LocalizedText, MachineView, RecommendationView, VoiceProfileId,
};
use device_profile_contract::{
    Backend, GpuVendor, HwClass as CoreClass, LocalLlm, PowerState, Profile, Recommendation,
    VoiceProfile,
};

fn backend(b: Backend) -> &'static str {
    match b {
        Backend::Cuda => "CUDA",
        Backend::Vulkan => "Vulkan",
        Backend::Cpu => "CPU",
    }
}

fn vendor(v: GpuVendor) -> &'static str {
    match v {
        GpuVendor::Nvidia => "NVIDIA",
        GpuVendor::Amd => "AMD",
        GpuVendor::Intel => "Intel",
        GpuVendor::Other => "inny",
    }
}

/// Profil + rekomendacja → DTO.
pub fn device_dto(p: &Profile, r: &Recommendation, name: String) -> DeviceProfile {
    let llm = match r.local_llm {
        LocalLlm::None => "brak (API)".to_owned(),
        size => format!("llama.cpp · {} · {size:?}", backend(r.llm_backend)),
    };
    DeviceProfile {
        machine: MachineView {
            id: p.machine_id.as_str().to_owned(),
            name,
            os: format!("{} {}", p.os.name, p.os.version).trim().to_owned(),
            cpu: CpuView {
                model: p.cpu.model.clone(),
                cores: p.cpu.physical_cores,
                threads: p.cpu.logical_cores,
            },
            ram_mb: u64::from(p.ram_mb),
            gpus: p
                .gpus
                .iter()
                .map(|g| GpuView {
                    vendor: vendor(g.vendor).to_owned(),
                    model: g.name.clone(),
                    vram_mb: u64::from(g.vram_mb),
                    backends: g.backends.iter().map(|b| backend(*b).to_owned()).collect(),
                    role: if p.primary_gpu().is_some_and(|primary| primary.id == g.id) {
                        "heavy"
                    } else if p.light_gpu().is_some_and(|light| light.id == g.id) {
                        "light"
                    } else {
                        "spare"
                    }
                    .to_owned(),
                    integrated: g.integrated,
                    shared_mb: u64::from(g.shared_mb),
                })
                .collect(),
            npu: p.npu.as_ref().map(|n| n.name.clone()),
            battery: p.battery.as_ref().map(|b| BatteryView {
                percent: b.percent.unwrap_or(0),
                on_ac: !matches!(p.power, PowerState::Battery { .. }),
            }),
        },
        recommendation: RecommendationView {
            hw_class: match r.class {
                CoreClass::Baseline => HwClass::Baseline,
                CoreClass::StandardAmd => HwClass::StandardAmd,
                CoreClass::LaptopCuda => HwClass::LaptopCuda,
                CoreClass::Strong => HwClass::Strong,
                CoreClass::Integrated => HwClass::Integrated,
                CoreClass::Unknown => HwClass::Unknown,
            },
            voice_profile: match r.voice_profile {
                VoiceProfile::A => VoiceProfileId::A,
                VoiceProfile::B => VoiceProfileId::B,
                VoiceProfile::C => VoiceProfileId::C,
                VoiceProfile::D => VoiceProfileId::D,
            },
            llm_backend: llm,
            // Kompromisy z `device-profile` są po polsku; wersja EN — w kolejnej fali i18n modułu.
            tradeoffs: r
                .tradeoffs
                .iter()
                .map(|t| LocalizedText::new(t.clone(), t.clone()))
                .collect(),
        },
        measured_at: dto::iso(chrono::Utc::now()),
    }
}

/// Odczyt w kształcie dla okna.
pub fn gpu_live_view(
    (cards, processes): (
        Vec<crate::gpu_mem::GpuCard>,
        Vec<crate::gpu_mem::GpuProcess>,
    ),
) -> GpuLive {
    GpuLive {
        available: true,
        cards: cards
            .into_iter()
            .map(|c| GpuLiveCard {
                index: c.index,
                name: c.name,
                total_mb: c.total_mb,
                used_mb: c.used_mb,
                free_mb: c.free_mb,
                load_pct: c.load_pct,
                temp_c: c.temp_c,
                power_w: c.power_w,
            })
            .collect(),
        processes: processes
            .into_iter()
            .map(|p| GpuLiveProcess {
                pid: p.pid,
                name: p.name,
                used_mb: p.used_mb,
                alfa: p.alfa,
            })
            .collect(),
    }
}
