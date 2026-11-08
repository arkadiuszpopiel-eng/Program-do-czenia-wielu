//! Rezydencja modeli głosu (`model-residency`): w trakcie rozmowy modele STT/TTS/VAD/końca tury są
//! oznaczane jako używane (nie wypiera ich dzierżawa o równym priorytecie), po rozmowie — zwalniane
//! do zwykłego cyklu (wyładowanie po bezczynności). Dzierżawy zakładają same silniki (`-impl`);
//! potok ich nie dubluje, tylko przypina.

use std::sync::Arc;

use model_residency_contract::{LeaseId, Residency};

/// Właściciele dzierżaw modeli głosu (jak w `module.toml` silników).
pub const VOICE_OWNERS: [&str; 4] = ["voice-stt", "voice-tts", "voice-vad", "voice-turn"];

/// Co ile ms odświeżać i doszukiwać się nowych dzierżaw (silniki ładują modele leniwie).
const REFRESH_MS: u64 = 1_000;

pub(crate) struct ResidencyPin {
    residency: Option<Arc<dyn Residency>>,
    pinned: Vec<LeaseId>,
    last_refresh: Option<u64>,
}

impl ResidencyPin {
    pub(crate) fn new(residency: Option<Arc<dyn Residency>>) -> Self {
        Self {
            residency,
            pinned: Vec::new(),
            last_refresh: None,
        }
    }

    /// Aktywna rozmowa → przypnij (i odświeżaj); koniec → odepnij.
    pub(crate) fn update(&mut self, active: bool, now_ms: u64) {
        let Some(r) = self.residency.clone() else {
            return;
        };
        if !active {
            for id in self.pinned.drain(..) {
                let _ = r.set_in_use(id, false);
            }
            self.last_refresh = None;
            return;
        }
        if self
            .last_refresh
            .is_some_and(|t| now_ms.saturating_sub(t) < REFRESH_MS)
        {
            return;
        }
        self.last_refresh = Some(now_ms);
        self.pinned.retain(|id| r.touch(*id).is_ok());
        for lease in r.snapshot().leases {
            let voice = VOICE_OWNERS.contains(&lease.request.owner.as_str());
            if voice && !self.pinned.contains(&lease.id) && r.set_in_use(lease.id, true).is_ok() {
                self.pinned.push(lease.id);
            }
        }
    }

    /// Przypięte dzierżawy (diagnostyka).
    pub(crate) fn pinned(&self) -> &[LeaseId] {
        &self.pinned
    }
}
