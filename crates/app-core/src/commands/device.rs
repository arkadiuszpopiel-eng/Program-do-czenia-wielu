//! Komendy `device_*`: profil sprzętu (`device-profile`) w kształcie UI.

use device_profile_contract::{
    Backend, GpuVendor, HwClass as CoreClass, LocalLlm, PowerState, Profile, Recommendation,
    VoiceProfile,
};

use crate::core::AppCore;
use crate::dto::{
    self, BatteryView, CpuView, DeviceProfile, GpuView, HwClass, LocalizedText, MachineView,
    RecommendationView, VoiceProfileId,
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
}
