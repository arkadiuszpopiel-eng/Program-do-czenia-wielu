//! Komendy `device_*`: profil sprzętu (`device-profile`) w kształcie UI.

use device_profile_contract::{
    Backend, GpuVendor, HwClass as CoreClass, LocalLlm, PowerState, Profile, Recommendation,
    VoiceProfile,
};

use crate::core::AppCore;
use crate::dto::{
    self, BatteryView, CpuView, DeviceProfile, GpuLive, GpuLiveCard, GpuLiveProcess, GpuView,
    HwClass, LocalizedText, MachineView, RecommendationView, VoiceProfileId,
};
use crate::error::AppError;

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

impl AppCore {
    fn device_now(&self) -> DeviceProfile {
        let device = &self.inner.device;
        let name = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Ten komputer".to_owned());
        device_dto(&device.current(), &device.recommend(), name)
    }

    /// `device_profile`.
    pub async fn device_profile(&self) -> Result<DeviceProfile, AppError> {
        Ok(self.device_now())
    }

    /// `device_measure`: ponowna detekcja sprzętu.
    pub async fn device_measure(&self) -> Result<DeviceProfile, AppError> {
        if let Err(e) = self.inner.device.refresh() {
            tracing::warn!(error = %e, "ponowna detekcja sprzętu nie powiodła się");
        }
        Ok(self.device_now())
    }

    /// `device_gpu_live`: karty graficzne na żywo (pamięć, obciążenie, temperatura, moc, programy).
    /// Odczyt uruchamia narzędzie sterownika, więc idzie poza wątkiem obsługi poleceń.
    pub async fn device_gpu_live(&self) -> Result<GpuLive, AppError> {
        let read = tokio::task::spawn_blocking(app_modules::gpu_mem::nvidia_live)
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        Ok(read.map_or_else(GpuLive::default, gpu_live_view))
    }
}

/// Odczyt w kształcie dla okna.
fn gpu_live_view(
    (cards, processes): (
        Vec<app_modules::gpu_mem::GpuCard>,
        Vec<app_modules::gpu_mem::GpuProcess>,
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
