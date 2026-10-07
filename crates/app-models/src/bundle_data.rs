//! Dane pakietów 1–6 (6 — wzorcowy, 1 — minimalny): skład (modele + silniki) i wymagania
//! sprzętowe z progiem „na styk”; uwagi o jakości — [`crate::bundle_quality`].

use app_api::dto::QualityNote;

use crate::bundle_quality::{quality_1, quality_2, quality_3, quality_4, quality_5, quality_6};

/// Bielik 4.5B v3.0 Instruct Q8_0 (z narzędziami agentek).
pub const LLM_BIG: &str = "bielik-4.5b-v3.0-instruct-q8_0";
/// Bielik 1.5B v3.0 Instruct Q8_0 (bez narzędzi).
pub const LLM_SMALL: &str = "bielik-1.5b-v3.0-instruct-q8_0";
/// Whisper large-v3-turbo q5_0.
pub const STT_TURBO: &str = "whisper-large-v3-turbo-q5_0";
/// Whisper small q5_1.
pub const STT_SMALL: &str = "whisper-small-q5_1";
/// Głos Piper pl_PL gosia (medium).
pub const TTS_PIPER: &str = "piper-pl_PL-gosia-medium";
/// Silero VAD.
pub const VAD: &str = "silero-vad";
/// Cechy słów wywoławczych openWakeWord.
pub const WAKE: &str = "openwakeword-features";
/// Model mówcy WeSpeaker (weryfikacja właściciela).
pub const SPEAKER: &str = "wespeaker-resnet34";
/// Embedder wyszukiwania (domyślny `lib-embed`).
pub const EMBED: &str = "multilingual-e5-small";
/// `llama-server` CUDA (z `cudart`).
pub const LLAMA_CUDA: &str = "sidecar-llama-cuda";
/// `llama-server` Vulkan.
pub const LLAMA_VULKAN: &str = "sidecar-llama-vulkan";
/// `llama-server` CPU.
pub const LLAMA_CPU: &str = "sidecar-llama-cpu";
/// `whisper-server` CUDA.
pub const WHISPER_CUDA: &str = "sidecar-whisper-cuda";
/// `whisper-server` CPU.
pub const WHISPER_CPU: &str = "sidecar-whisper-cpu";
/// Silnik `piper`.
pub const PIPER: &str = "sidecar-piper";

/// Wartość nominalna (GB, do opisu) i próg w MB z tolerancją raportowania systemu — te same
/// progi co `device-profile` (16 GB RAM → ~15,9 GB, karta 8 GB → ~8176 MB, RTX 4050 6 GB → 5921 MB).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gb {
    /// Nominalnie (GB).
    pub nominal: u32,
    /// Próg (MB).
    pub min_mb: u64,
}

const RAM_16: Gb = Gb {
    nominal: 16,
    min_mb: 15_000,
};
const RAM_12: Gb = Gb {
    nominal: 12,
    min_mb: 11_000,
};
const RAM_8: Gb = Gb {
    nominal: 8,
    min_mb: 7_000,
};
const RAM_6: Gb = Gb {
    nominal: 6,
    min_mb: 5_500,
};
const VRAM_8: Gb = Gb {
    nominal: 8,
    min_mb: 7_500,
};
const VRAM_6: Gb = Gb {
    nominal: 6,
    min_mb: 5_500,
};
const VRAM_4: Gb = Gb {
    nominal: 4,
    min_mb: 3_500,
};

/// Wymagania: RAM i moc obliczeniowa (karta, procesor albo jedno z nich).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Need {
    /// Pamięć RAM.
    pub ram: Gb,
    /// Pamięć karty (CUDA albo Vulkan).
    pub vram: Option<Gb>,
    /// Rdzenie fizyczne procesora.
    pub cores: Option<u32>,
    /// Karta konieczna (inaczej `vram` i `cores` to alternatywy).
    pub gpu_required: bool,
}

const fn need(ram: Gb, vram: Option<Gb>, cores: Option<u32>, gpu_required: bool) -> Need {
    Need {
        ram,
        vram,
        cores,
        gpu_required,
    }
}

/// Próg „na styk”: zadziała z kompromisem opisanym w `reason` (PL, EN).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tight {
    /// Wymagania złagodzone.
    pub need: Need,
    /// Kompromis.
    pub reason: (&'static str, &'static str),
}

/// Silnik pakietu (wariant CUDA / Vulkan / CPU dobierany dla maszyny).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// `llama-server` (model rozmowy).
    Llama,
    /// `whisper-server` (rozpoznawanie mowy).
    Whisper,
    /// `piper` (głos).
    Piper,
}

/// Definicja pakietu.
#[derive(Debug, Clone, Copy)]
pub struct Def {
    /// Identyfikator (komendy UI).
    pub id: &'static str,
    /// Ocena 1–6.
    pub rating: u8,
    /// Nazwa (PL, EN).
    pub name: (&'static str, &'static str),
    /// Opis (PL, EN).
    pub summary: (&'static str, &'static str),
    /// Wymagania.
    pub need: Need,
    /// Próg „na styk”.
    pub tight: Option<Tight>,
    /// Modele (identyfikatory katalogu).
    pub models: &'static [&'static str],
    /// Silniki.
    pub engines: &'static [Engine],
    /// Uwagi o jakości.
    pub quality: fn() -> Vec<QualityNote>,
}

const ALL_ENGINES: &[Engine] = &[Engine::Llama, Engine::Whisper, Engine::Piper];

/// Pakiety od najlepszego (6) do najsłabszego (1).
pub const DEFS: [Def; 6] = [
    Def {
        id: "bundle-reference",
        rating: 6,
        name: ("Wzorcowy", "Reference"),
        summary: (
            "Pełna Alfa na tym komputerze: największy model rozmowy Bielik 4.5B (z narzędziami agentek), dokładne rozpoznawanie mowy Whisper turbo, głos Piper, wykrywanie mowy, cechy słowa wywoławczego, weryfikacja Twojego głosu i wyszukiwanie znaczeniowe w pamięci. Silniki na kartę graficzną z zapasem na procesor.",
            "Full Alfa on this computer: the largest chat model Bielik 4.5B (with the agents' tools), accurate Whisper turbo speech recognition, the Piper voice, voice activity detection, wake-word features, verification of your voice and semantic memory search. GPU engines with a CPU fallback.",
        ),
        need: need(RAM_16, Some(VRAM_8), None, true),
        tight: Some(Tight {
            need: need(RAM_16, Some(VRAM_6), None, true),
            reason: (
                "Karta ma mniej niż 8 GB pamięci: Bielik 4.5B zmieści się na niej tylko częściowo (reszta warstw w RAM), więc odpowiedzi będą wolniejsze niż na karcie 8 GB.",
                "The GPU has less than 8 GB of memory: Bielik 4.5B fits on it only partly (the remaining layers in RAM), so replies are slower than on an 8 GB card.",
            ),
        }),
        models: &[LLM_BIG, STT_TURBO, TTS_PIPER, VAD, WAKE, SPEAKER, EMBED],
        engines: ALL_ENGINES,
        quality: quality_6,
    },
    Def {
        id: "bundle-very-good",
        rating: 5,
        name: ("Bardzo dobry", "Very good"),
        summary: (
            "Jak Wzorcowy, ale bez słowa wywoławczego i weryfikacji głosu. Na karcie 6 GB model 4.5B dzieli się między kartę i procesor — odpowiedzi trochę wolniejsze.",
            "Like Reference, but without the wake word and voice verification. On a 6 GB GPU the 4.5B model is split between the GPU and the CPU — slightly slower replies.",
        ),
        need: need(RAM_16, Some(VRAM_6), None, true),
        tight: Some(Tight {
            need: need(RAM_16, Some(VRAM_4), None, true),
            reason: (
                "Karta ma mniej niż 6 GB pamięci: większa część Bielika 4.5B liczy się na procesorze — odpowiedzi wyraźnie wolniejsze.",
                "The GPU has less than 6 GB of memory: most of Bielik 4.5B runs on the CPU — noticeably slower replies.",
            ),
        }),
        models: &[LLM_BIG, STT_TURBO, TTS_PIPER, VAD, EMBED],
        engines: ALL_ENGINES,
        quality: quality_5,
    },
    Def {
        id: "bundle-good",
        rating: 4,
        name: ("Dobry", "Good"),
        summary: (
            "Lżejszy model rozmowy Bielik 1.5B (rozmowa bez narzędzi agentek) z dokładnym rozpoznawaniem mowy Whisper turbo, głosem Piper i wyszukiwaniem znaczeniowym. Działa na karcie 4 GB albo na mocnym procesorze.",
            "The lighter chat model Bielik 1.5B (conversation without the agents' tools) with accurate Whisper turbo speech recognition, the Piper voice and semantic search. Runs on a 4 GB GPU or a strong CPU.",
        ),
        need: need(RAM_12, Some(VRAM_4), Some(8), false),
        tight: Some(Tight {
            need: need(RAM_12, None, Some(6), false),
            reason: (
                "Bez karty ≥ 4 GB i z procesorem poniżej 8 rdzeni Whisper turbo rozpoznaje mowę wolno — rozmowa głosowa z opóźnieniem.",
                "Without a ≥ 4 GB GPU and with fewer than 8 CPU cores, Whisper turbo recognises speech slowly — delayed voice conversation.",
            ),
        }),
        models: &[LLM_SMALL, STT_TURBO, TTS_PIPER, VAD, EMBED],
        engines: ALL_ENGINES,
        quality: quality_4,
    },
    Def {
        id: "bundle-balanced",
        rating: 3,
        name: ("Zrównoważony", "Balanced"),
        summary: (
            "Bielik 1.5B, mniejszy model rozpoznawania mowy Whisper small i głos Piper — rozmowa głosowa bez karty graficznej, wolniejsza niż na karcie. Wyszukiwanie w pamięci po słowach (bez modelu znaczeniowego).",
            "Bielik 1.5B, the smaller Whisper small speech model and the Piper voice — voice conversation without a GPU, slower than on a GPU. Memory search by words (no semantic model).",
        ),
        need: need(RAM_8, Some(VRAM_4), Some(6), false),
        tight: Some(Tight {
            need: need(RAM_8, None, Some(4), false),
            reason: (
                "Procesor ma mniej niż 6 rdzeni: rozpoznawanie mowy będzie bardzo wolne — wygodniej pisać.",
                "The CPU has fewer than 6 cores: speech recognition will be very slow — typing is more convenient.",
            ),
        }),
        models: &[LLM_SMALL, STT_SMALL, TTS_PIPER, VAD],
        engines: ALL_ENGINES,
        quality: quality_3,
    },
    Def {
        id: "bundle-light",
        rating: 2,
        name: ("Lekki", "Light"),
        summary: (
            "Bielik 1.5B i rozpoznawanie mowy Whisper small: mówisz do Alfy, a ona odpowiada tekstem (bez syntezy mowy).",
            "Bielik 1.5B and Whisper small speech recognition: you speak to Alfa and she replies in text (no speech synthesis).",
        ),
        need: need(RAM_8, None, None, false),
        tight: None,
        models: &[LLM_SMALL, STT_SMALL, VAD],
        engines: &[Engine::Llama, Engine::Whisper],
        quality: quality_2,
    },
    Def {
        id: "bundle-minimal",
        rating: 1,
        name: ("Minimalny", "Minimal"),
        summary: (
            "Tylko rozmowa tekstowa z Bielikiem 1.5B — najmniej miejsca na dysku i pamięci.",
            "Text chat with Bielik 1.5B only — the least disk space and memory.",
        ),
        need: need(RAM_6, None, None, false),
        tight: None,
        models: &[LLM_SMALL],
        engines: &[Engine::Llama],
        quality: quality_1,
    },
];
