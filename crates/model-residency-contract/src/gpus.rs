//! Karty graficzne w zarządcy (C6.6): osobna pula pamięci każdej karty, wspólna pula RAM.
//!
//! Klucz karty to [`GpuId`] — `GpuInfo::id` z `device-profile` (kolejność adapterów, stała
//! w obrębie maszyny). LUID adaptera zmienia się po ponownym uruchomieniu systemu, więc kluczem nie
//! jest; kartę z licznikami pamięci łączy profil urządzenia (`GpuInfo::luid`). Pula karty głównej
//! to [`Budget`] (jak dotąd: rekomendacja `device-profile` z nakładką `[machine.residency]`),
//! pozostałe karty mają własne pule ([`GpuPool`]). Żądanie bez karty albo z kartą nieznaną trafia
//! na kartę główną — maszyna z jedną kartą działa dokładnie tak jak przed podziałem.

use std::fmt;

use device_profile_contract::{Backend, DESKTOP_RESERVE_MB, GpuInfo, Profile};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::Budget;

/// Karta graficzna w zarządcy: `GpuInfo::id` z `device-profile`.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(transparent)]
pub struct GpuId(pub u32);

impl GpuId {
    /// Identyfikator karty z profilu urządzenia.
    pub fn of(gpu: &GpuInfo) -> Self {
        Self(gpu.id)
    }
}

impl fmt::Display for GpuId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gpu-{}", self.0)
    }
}

/// Karta z własną pamięcią mniejszą niż tyle MB nie mieści STT i ciężkiego TTS naraz — ten sam próg
/// co budżet karty głównej w `device-profile` (8 GB z tolerancją raportowania).
const EXCLUSIVE_BELOW_MB: u32 = 7_500;

/// Pula pamięci jednej karty (MB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct GpuPool {
    /// Karta.
    pub gpu: GpuId,
    /// Pamięć karty dla modeli (MB, po rezerwie pulpitu).
    pub vram_mb: u32,
    /// Rezerwa pulpitu (MB) — informacyjnie, już odjęta od `vram_mb`.
    pub desktop_reserve_mb: u32,
    /// STT i ciężki TTS nie mogą być naraz na tej karcie.
    pub stt_tts_exclusive: bool,
    /// Karta bez własnej pamięci (zintegrowana z małą rezerwą z BIOS-u): jej pamięć jest częścią
    /// RAM, więc dzierżawa na niej zajmuje także pulę RAM maszyny.
    pub shared: bool,
}

impl GpuPool {
    /// Pula karty z profilu urządzenia: karta z własną pamięcią (osobna albo zintegrowana z dużą
    /// rezerwą z BIOS-u) — pamięć minus rezerwa pulpitu, STT i ciężki TTS na zmianę poniżej 8 GB;
    /// zintegrowana z małą rezerwą — część RAM ([`GpuInfo::usable_memory_mb`]), współdzielona,
    /// STT i ciężki TTS na zmianę (jak klasa „karta zintegrowana” w `device-profile`).
    pub fn from_gpu(gpu: &GpuInfo, ram_mb: u32) -> Self {
        let id = GpuId::of(gpu);
        if gpu.has_memory_pool() {
            let reserve = if gpu.vram_mb > 0 {
                DESKTOP_RESERVE_MB
            } else {
                0
            };
            Self {
                gpu: id,
                vram_mb: gpu.vram_mb.saturating_sub(reserve),
                desktop_reserve_mb: reserve,
                stt_tts_exclusive: gpu.vram_mb < EXCLUSIVE_BELOW_MB,
                shared: false,
            }
        } else {
            Self {
                gpu: id,
                vram_mb: gpu.usable_memory_mb(ram_mb),
                desktop_reserve_mb: 0,
                stt_tts_exclusive: true,
                shared: true,
            }
        }
    }

    /// Pula karty głównej z budżetu maszyny (budżet karty głównej jest już po emulacji).
    pub fn primary(gpu: GpuId, budget: Budget) -> Self {
        Self {
            gpu,
            vram_mb: budget.vram_mb,
            desktop_reserve_mb: budget.desktop_reserve_mb,
            stt_tts_exclusive: budget.stt_tts_exclusive,
            shared: false,
        }
    }

    /// Pula ograniczona emulacją słabszej maszyny — każda karta najwyżej tyle, ile emulowana
    /// (tak samo jak `Profile::emulate` ogranicza pamięć każdej karty).
    #[must_use]
    pub fn limited(self, emulated: Budget) -> Self {
        Self {
            vram_mb: self.vram_mb.min(emulated.vram_mb),
            desktop_reserve_mb: self.desktop_reserve_mb.max(emulated.desktop_reserve_mb),
            stt_tts_exclusive: self.stt_tts_exclusive || emulated.stt_tts_exclusive,
            ..self
        }
    }
}

/// Karty maszyny: główna (jej pula to [`Budget`]) i pozostałe z własnymi pulami.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GpuLayout {
    /// Karta główna.
    pub primary: GpuId,
    /// Pozostałe karty (bez głównej, bez powtórzeń — [`GpuLayout::normalized`]).
    pub others: Vec<GpuPool>,
}

impl GpuLayout {
    /// Jedna karta (albo żadna): wszystkie dzierżawy na karcie w budżecie karty głównej.
    pub fn single(primary: GpuId) -> Self {
        Self {
            primary,
            others: Vec::new(),
        }
    }

    /// Karty z profilu urządzenia: główna jak `Profile::primary_gpu`, pozostałe z obsługiwanym
    /// backendem (karta bez backendu obliczeń nie dostaje puli — żądanie z nią trafia na główną).
    /// Bez karty — [`GpuLayout::default`].
    pub fn from_profile(profile: &Profile) -> Self {
        let Some(primary) = profile.primary_gpu() else {
            return Self::default();
        };
        let others = profile
            .gpus
            .iter()
            .filter(|g| g.id != primary.id && !g.backends.is_empty())
            .map(|g| GpuPool::from_gpu(g, profile.ram_mb))
            .collect();
        Self {
            primary: GpuId::of(primary),
            others,
        }
        .normalized()
    }

    /// Bez karty głównej wśród pozostałych i bez powtórzeń (wygrywa pierwsza pula danej karty).
    #[must_use]
    pub fn normalized(mut self) -> Self {
        let mut seen = vec![self.primary];
        self.others.retain(|p| {
            let fresh = !seen.contains(&p.gpu);
            seen.push(p.gpu);
            fresh
        });
        self
    }
}

/// Karta, na której liczy silnik z backendem `backend` — klucz [`crate::LeaseRequest::gpu`]
/// (C6.6). Ta sama decyzja, która wybiera kartę silnika: karta przypięta po nazwie (`--device`
/// `llama-server`: nazwa dopasowana jak w [`same_card_name`], tylko karta obsługująca backend —
/// inaczej serwer startuje bez przypięcia), inaczej karta główna profilu, gdy obsługuje backend
/// (Qwen3-ASR i model rozmowy na głównej, CUDA na laptopie z RTX), inaczej największa karta z tym
/// backendem (np. CUDA przy karcie głównej AMD). Procesor albo maszyna bez kart — `None`
/// (dzierżawa na CPU albo na karcie głównej, jak dotąd).
pub fn card_for(profile: &Profile, backend: Backend, name: Option<&str>) -> Option<GpuId> {
    if backend == Backend::Cpu {
        return None;
    }
    let named = name.and_then(|n| {
        profile
            .gpus
            .iter()
            .filter(|g| g.backends.contains(&backend))
            .find(|g| same_card_name(&g.name, n))
    });
    if let Some(gpu) = named {
        return Some(GpuId::of(gpu));
    }
    let primary = profile.primary_gpu();
    if let Some(gpu) = primary.filter(|g| g.backends.contains(&backend)) {
        return Some(GpuId::of(gpu));
    }
    profile
        .gpus
        .iter()
        .filter(|g| g.backends.contains(&backend))
        .max_by_key(|g| {
            (
                u8::from(g.has_memory_pool()),
                g.vram_mb,
                std::cmp::Reverse(g.id),
            )
        })
        .or(primary)
        .map(GpuId::of)
}

/// Nazwa karty bez wielkości liter, znaków `(R)`/`(TM)` i nadmiarowych odstępów.
fn normalized_name(name: &str) -> String {
    name.to_ascii_lowercase()
        .replace("(r)", "")
        .replace("(tm)", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Czy dwie nazwy wskazują tę samą kartę: po ujednoliceniu (wielkość liter, `(R)`, `(TM)`,
/// odstępy) jedna zawiera drugą. Jedna reguła dla [`card_for`] i dopasowania urządzeń
/// `llama-server` (`--list-devices`), żeby obie strony wybierały tę samą kartę. Pusta nazwa nie
/// pasuje do niczego.
pub fn same_card_name(a: &str, b: &str) -> bool {
    let (a, b) = (normalized_name(a), normalized_name(b));
    !a.is_empty() && !b.is_empty() && (a.contains(&b) || b.contains(&a))
}

/// Stan jednej karty w migawce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GpuState {
    /// Pula obowiązująca (po emulacji).
    pub pool: GpuPool,
    /// Czy to karta główna.
    pub primary: bool,
    /// Pamięć karty zajęta przez dzierżawy (MB).
    pub used_mb: u32,
}

impl GpuState {
    /// Wolna pamięć puli karty (MB).
    pub fn free_mb(&self) -> u32 {
        self.pool.vram_mb.saturating_sub(self.used_mb)
    }
}

#[cfg(test)]
#[path = "gpus_tests.rs"]
mod tests;
