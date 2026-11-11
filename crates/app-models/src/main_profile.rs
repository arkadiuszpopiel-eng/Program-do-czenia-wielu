//! Profil główny8B dla osobnej karty12GB; mały Bielik pozostaje pomocnikiem.
use crate::bundle_data::{Def, LLM_BIG, LLM_SMALL};
use crate::bundles::Machine;
use app_api::dto::{LocalizedText, QualityNote};

pub const MAIN: &str = providers_local_impl::MAIN_MODEL;

pub fn enabled(machine: &Machine) -> bool {
    machine.gpu.is_some() && machine.vram_mb >= 11_000
}

pub fn model_note(id: &str) -> LocalizedText {
    match id {
        MAIN => LocalizedText::new(
            "Główny model8B. Docelowo karta12GB; kontekst8k i rezerwy mowy. Wydajność wymaga pomiaru na tym komputerze.",
            "Main8B model. Targets a12GB GPU with8k context and speech reserves. Performance needs measurement on this computer.",
        ),
        LLM_SMALL => LocalizedText::new(
            "Model pomocniczy do lekkich streszczeń i porządków. Nie zastępuje głównego8–12B; bez narzędzi.",
            "Auxiliary model for light summaries and housekeeping. Does not replace the main8–12B model; no tools.",
        ),
        _ => LocalizedText::new(
            "Lokalny model rozmowy (llama.cpp). Wymaga sidecara llama-server.",
            "Local chat model (llama.cpp). Requires the llama-server sidecar.",
        ),
    }
}

pub fn members(list: &mut [(&'static str, bool)], machine: &Machine) {
    if enabled(machine) {
        for (id, _) in list {
            if [LLM_BIG, LLM_SMALL].contains(id) {
                *id = MAIN;
            }
        }
    }
}

pub fn summary(def: &Def, machine: &Machine) -> LocalizedText {
    if enabled(machine) {
        LocalizedText::new(
            "Główny Qwen3-8B Q4_K_M z kontekstem8k na karcie12GB; pozostałe składniki głosu i pamięci według listy tego pakietu. Wydajność i jakość wymagają odbioru na komputerze.",
            "Main Qwen3-8B Q4_K_M with8k context on a12GB GPU; speech and memory components are listed in this bundle. Performance and quality need acceptance on this computer.",
        )
    } else {
        LocalizedText::new(def.summary.0, def.summary.1)
    }
}

pub fn quality(def: &Def, machine: &Machine) -> Vec<QualityNote> {
    let mut notes = (def.quality)();
    if enabled(machine) {
        notes.retain(|n| !["Model rozmowy", "Wydajność"].contains(&n.aspect.pl.as_str()));
        notes.insert(0,QualityNote {
            aspect: LocalizedText::new("Model rozmowy","Chat model"),
            standard: "ISO/IEC 25059:2023".into(),
            text: LocalizedText::new(
                "Qwen3-8B Q4_K_M: model główny, bez thinking. Szacunek przy8k:6497MiB wagi/narzut/KV,1500MiB STT,600MiB TTS i768MiB ekranu; zostaje2923MiB z12GiB. To rachunek pamięci, nie pomiar jakości aniGPU. Poprzednia próbaCoreCPU przekroczyła limit.",
                "Qwen3-8B Q4_K_M: main model, thinking disabled. At8k:6497MiB weights/overhead/KV,1500MiB STT,600MiB TTS and768MiB display;2923MiB remains from12GiB. Memory estimate, not a quality orGPU measurement. Previous CoreCPU trial exceeded its deadline."),
        });
    }
    notes
}
