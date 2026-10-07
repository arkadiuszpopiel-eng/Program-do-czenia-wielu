//! Nakładka per maszyna (`config/machine/<id>.toml`, PLAN §3.5) — tylko rzeczy fizyczne:
//! nadpisanie klasy i profilu głosu, limity, zachowanie na baterii. Nigdy agentki ani uprawnienia.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::rules::{HwClass, Recommendation, VoiceProfile, VoiceVariant, classify, recommend_as};
use crate::types::{PowerState, Profile, ResourceLimits};

/// Wybór profilu głosu w nakładce (`[machine.voice] profile = "auto" | "A" | "B" | "C" | "D"`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum VoiceChoice {
    /// Według rekomendacji.
    #[default]
    #[serde(rename = "auto")]
    Auto,
    /// Profil A.
    A,
    /// Profil B.
    B,
    /// Profil C.
    C,
    /// Profil D.
    D,
}

/// Nakładka maszyny. Nadpisanie użytkownika ma pierwszeństwo, ale kompromis trafia do `tradeoffs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct MachineOverlay {
    /// Nazwa maszyny nadana przez użytkownika.
    pub name: Option<String>,
    /// Nadpisana klasa sprzętu.
    pub hw_class_override: Option<HwClass>,
    /// Profil głosu.
    pub voice_profile: VoiceChoice,
    /// Limity zasobów dla modeli (zawężają budżet rezydencji).
    pub limits: Option<ResourceLimits>,
    /// Na baterii ograniczać lokalne modele (`[machine.battery] reduce_local_models`).
    pub reduce_local_models_on_battery: bool,
}

impl Default for MachineOverlay {
    fn default() -> Self {
        Self {
            name: None,
            hw_class_override: None,
            voice_profile: VoiceChoice::Auto,
            limits: None,
            reduce_local_models_on_battery: true,
        }
    }
}

impl VoiceChoice {
    fn fixed(self) -> Option<VoiceProfile> {
        match self {
            VoiceChoice::Auto => None,
            VoiceChoice::A => Some(VoiceProfile::A),
            VoiceChoice::B => Some(VoiceProfile::B),
            VoiceChoice::C => Some(VoiceProfile::C),
            VoiceChoice::D => Some(VoiceProfile::D),
        }
    }
}

/// Rekomendacja z uwzględnieniem nakładki maszyny.
pub fn apply_overlay(profile: &Profile, overlay: &MachineOverlay) -> Recommendation {
    let mut base = profile.clone();
    if !overlay.reduce_local_models_on_battery && base.on_battery() {
        base.power = PowerState::Ac;
    }
    let class = overlay.hw_class_override.unwrap_or_else(|| classify(&base));
    let mut rec = recommend_as(&base, class);
    if let Some(forced) = overlay.hw_class_override {
        rec.tradeoffs.push(format!(
            "Klasa sprzętu nadpisana ręcznie ({forced:?}); rekomendacja automatyczna: {:?}.",
            classify(&base)
        ));
    }
    if let Some(limits) = overlay.limits {
        rec.residency.vram_mb = rec.residency.vram_mb.min(limits.vram_mb);
        rec.residency.ram_mb = rec.residency.ram_mb.min(limits.ram_mb);
    }
    if let Some(voice) = overlay.voice_profile.fixed()
        && voice != rec.voice_profile
    {
        rec.tradeoffs.push(voice_tradeoff(voice, rec.class));
        rec.voice_variant = match voice {
            VoiceProfile::D => rec.voice_variant.or(Some(match rec.stt_backend {
                crate::types::Backend::Cuda => VoiceVariant::Cuda,
                _ => VoiceVariant::Amd16,
            })),
            _ => None,
        };
        rec.voice_profile = voice;
    }
    rec
}

fn voice_tradeoff(voice: VoiceProfile, class: HwClass) -> String {
    match voice {
        VoiceProfile::A => {
            "Profil A (ręcznie): bez chmury; jakość rozmowy ograniczona rozmiarem lokalnego LLM."
                .into()
        }
        VoiceProfile::B => "Profil B (ręcznie): rozmowa przez API, STT lokalnie.".into(),
        VoiceProfile::C => {
            "Profil C (ręcznie): audio trafia do chmury (tag prywatności), najniższe opóźnienie."
                .into()
        }
        VoiceProfile::D if matches!(class, HwClass::Baseline | HwClass::Unknown) => {
            "Profil D (ręcznie) na słabszej maszynie: ryzyko braku VRAM i opóźnień ponad budżet."
                .into()
        }
        VoiceProfile::D => "Profil D (ręcznie): pełniejszy lokalny stos.".into(),
    }
}
