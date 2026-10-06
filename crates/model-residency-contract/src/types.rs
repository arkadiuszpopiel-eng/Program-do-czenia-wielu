//! Typy zarządcy rezydencji: budżet, żądanie dzierżawy, dzierżawa, tryb, stan, błędy.

use std::fmt;

use device_profile_contract::ResidencyBudget;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Rodzaj modelu (PLAN §3.4: STT, TTS, LLM, embedder, VAD).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ModelRole {
    /// Rozpoznawanie mowy (whisper.cpp).
    Stt,
    /// Synteza mowy.
    Tts,
    /// Lokalny LLM (llama.cpp).
    Llm,
    /// Model osadzeń (wyszukiwanie).
    Embedder,
    /// VAD / koniec tury (małe, zwykle CPU).
    Vad,
}

/// Priorytet dzierżawy: `VoiceRt` > `Conversation` > `Background` (kolejność `Ord`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    /// Zadania tła (konsolidacja, indeksowanie).
    Background,
    /// Rozmowa tekstowa.
    Conversation,
    /// Głos w czasie rzeczywistym.
    VoiceRt,
}

/// Preferencja umiejscowienia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    /// Tylko GPU (brak wersji CPU) — w trybie gry odrzucane.
    GpuOnly,
    /// GPU, a gdy się nie da — CPU.
    GpuPreferred,
    /// GPU tylko z wolnego miejsca: bez wypierania innych dzierżaw z budżetu (wymiana STT ↔ ciężki
    /// TTS dozwolona), inaczej CPU; GPU z wypieraniem dopiero, gdy CPU też się nie da. Dla modeli
    /// z użyteczną wersją CPU, których załadowanie nie powinno wyrzucać z karty modelu potrzebnego
    /// w tej samej rozmowie (STT obok lokalnego LLM na laptopie 6 GB: bez przeładowań LLM).
    GpuIfFree,
    /// Tylko CPU.
    CpuOnly,
}

/// Faktyczne umiejscowienie dzierżawy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Device {
    /// Karta graficzna (VRAM).
    Gpu,
    /// Procesor (RAM).
    Cpu,
}

/// Budżet maszyny do rozdania między modele (po odjęciu rezerwy pulpitu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Budget {
    /// VRAM dla modeli (MB).
    pub vram_mb: u32,
    /// RAM dla modeli (MB).
    pub ram_mb: u32,
    /// Rezerwa VRAM pulpitu (MB) — informacyjnie, już odjęta od `vram_mb`.
    pub desktop_reserve_mb: u32,
    /// STT i ciężki TTS nie mogą być jednocześnie na GPU (laptop 6 GB).
    pub stt_tts_exclusive: bool,
}

impl Budget {
    /// Budżet z rekomendacji `device-profile`.
    pub fn from_device(b: &ResidencyBudget) -> Self {
        Self {
            vram_mb: b.vram_mb,
            ram_mb: b.ram_mb,
            desktop_reserve_mb: b.desktop_reserve_mb,
            stt_tts_exclusive: b.stt_tts_exclusive,
        }
    }

    /// Mniejszy z dwóch budżetów (emulacja słabszej maszyny).
    #[must_use]
    pub fn min(self, other: Budget) -> Budget {
        Budget {
            vram_mb: self.vram_mb.min(other.vram_mb),
            ram_mb: self.ram_mb.min(other.ram_mb),
            desktop_reserve_mb: self.desktop_reserve_mb.max(other.desktop_reserve_mb),
            stt_tts_exclusive: self.stt_tts_exclusive || other.stt_tts_exclusive,
        }
    }
}

/// Zużycie zasobów (MB).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Usage {
    /// VRAM.
    pub vram_mb: u32,
    /// RAM.
    pub ram_mb: u32,
}

/// Identyfikator dzierżawy (monotoniczny w instancji zarządcy).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct LeaseId(pub u64);

impl fmt::Display for LeaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "lease-{}", self.0)
    }
}

/// Żądanie dzierżawy (szacunki z manifestu modelu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LeaseRequest {
    /// Moduł właściciela (np. `providers-local`).
    pub owner: String,
    /// Model (np. `bielik-4.5b-v3.0-instruct-q8_0`).
    pub model: String,
    /// Rodzaj.
    pub role: ModelRole,
    /// Priorytet.
    pub priority: Priority,
    /// Preferencja umiejscowienia.
    pub placement: Placement,
    /// VRAM na GPU (wagi + KV cache).
    pub vram_mb: u32,
    /// RAM procesu przy modelu na GPU.
    pub ram_mb: u32,
    /// RAM przy modelu na CPU (wagi w RAM).
    pub cpu_ram_mb: u32,
    /// Zwolnienie po bezczynności (ms); 0 = nigdy.
    pub idle_unload_ms: u64,
}

impl LeaseRequest {
    /// Zapotrzebowanie na danym urządzeniu.
    pub fn need(&self, device: Device) -> Usage {
        match device {
            Device::Gpu => Usage {
                vram_mb: self.vram_mb,
                ram_mb: self.ram_mb,
            },
            Device::Cpu => Usage {
                vram_mb: 0,
                ram_mb: self.cpu_ram_mb,
            },
        }
    }
}

/// Przyznana dzierżawa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Lease {
    /// Identyfikator.
    pub id: LeaseId,
    /// Żądanie źródłowe.
    pub request: LeaseRequest,
    /// Umiejscowienie.
    pub device: Device,
    /// Chwila przyznania (ms zegara zarządcy).
    pub granted_at_ms: u64,
    /// Ostatnie użycie (`touch`/`set_in_use`).
    pub last_used_ms: u64,
    /// Czy model jest właśnie używany (np. tura głosu) — nie jest wtedy eksmitowany przez równy priorytet.
    pub in_use: bool,
}

impl Lease {
    /// Zużycie dzierżawy.
    pub fn usage(&self) -> Usage {
        self.request.need(self.device)
    }

    /// Priorytet.
    pub fn priority(&self) -> Priority {
        self.request.priority
    }
}

/// Powód odebrania dzierżawy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum RevokeReason {
    /// Wyparta przez dzierżawę o wyższym priorytecie albo LRU przy równym.
    Preempted {
        /// Dzierżawa, która zajęła miejsce.
        by: LeaseId,
    },
    /// STT i ciężki TTS nie mogą być naraz na GPU.
    Exclusive {
        /// Dzierżawa, która zajęła miejsce.
        by: LeaseId,
    },
    /// Tryb gry — GPU zwolnione, brak miejsca w RAM na przeniesienie.
    Gaming,
    /// Tryb baterii — modele tła wyładowane.
    Battery,
    /// Budżet zmniejszony (emulacja, zmiana sprzętu).
    BudgetShrunk,
    /// Bezczynność dłuższa niż `idle_unload_ms`.
    Idle,
}

/// Odebrana dzierżawa (właściciel musi wyładować model).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Revocation {
    /// Dzierżawa w chwili odebrania.
    pub lease: Lease,
    /// Powód.
    pub reason: RevokeReason,
}

/// Wynik udanego `acquire`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Grant {
    /// Nowa dzierżawa.
    pub lease: Lease,
    /// Dzierżawy odebrane, by zrobić miejsce (w kolejności eksmisji).
    pub evicted: Vec<Revocation>,
}

/// Tryb pracy (flagi łączą się: laptop na baterii z grą ma oba).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Mode {
    /// Pełny ekran / gra: nowe dzierżawy GPU odrzucane, istniejące przenoszone na CPU.
    pub gaming: bool,
    /// Bateria: modele tła nie są ładowane.
    pub battery: bool,
    /// Emulacja słabszej maszyny (np. baseline 8 GB) — budżet = min(rzeczywisty, ten).
    pub emulated: Option<Budget>,
}

impl Mode {
    /// Tryb normalny.
    pub fn normal() -> Self {
        Self::default()
    }
}

/// Migawka stanu (UI: co jest gdzie, ile wolne).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResidencyState {
    /// Budżet obowiązujący (po emulacji).
    pub budget: Budget,
    /// Zajęte.
    pub used: Usage,
    /// Tryb.
    pub mode: Mode,
    /// Dzierżawy (rosnąco po id).
    pub leases: Vec<Lease>,
}

/// Błędy zarządcy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum ResidencyError {
    /// Zasoby trzymają dzierżawy o **wyższym lub równym** priorytecie w użyciu — spróbuj później.
    #[error(
        "brak miejsca: zasoby zajmują dzierżawy o wyższym lub równym priorytecie ({blockers:?})"
    )]
    Wait {
        /// Dzierżawy blokujące (nigdy o niższym priorytecie niż żądanie).
        blockers: Vec<LeaseId>,
    },
    /// Model większy niż cały budżet maszyny.
    #[error("model `{model}` nie mieści się w budżecie (VRAM {vram_mb} MB, RAM {ram_mb} MB)")]
    TooLarge {
        /// Model.
        model: String,
        /// Budżet VRAM.
        vram_mb: u32,
        /// Budżet RAM.
        ram_mb: u32,
    },
    /// Tryb gry — dzierżawa wyłącznie GPU jest niedozwolona.
    #[error("tryb gry: GPU niedostępne dla nowych modeli")]
    Gaming,
    /// Tryb baterii — modele tła nie są ładowane.
    #[error("tryb baterii: modele tła wstrzymane")]
    Battery,
    /// Niepoprawne żądanie.
    #[error("niepoprawne żądanie dzierżawy: {0}")]
    Invalid(String),
    /// Nieznana (zwolniona lub odebrana) dzierżawa.
    #[error("nieznana dzierżawa {0}")]
    UnknownLease(LeaseId),
}
