//! Qwen3-ASR 1.7B — rozpoznawanie mowy główne (zadanie P1): model i projektor dźwięku z przypiętej
//! wersji repozytorium `ggml-org/Qwen3-ASR-1.7B-GGUF`, z rozmiarami i sumami SHA-256 (zgodne z listą
//! plików wydawcy; sprawdzone też na pobranych plikach). Wymaga `llama-server` b11559 lub nowszego.

use app_api::dto::ModelItemKind;

use crate::catalog::{FileSpec, Install, ItemSpec, Root, note};

const HF_QWEN_ASR: &str = "https://huggingface.co/ggml-org/Qwen3-ASR-1.7B-GGUF/resolve/36a678687ba7d07a74ca70ccb0e36902e005fb80";

/// Plik modelu.
pub const MODEL_FILE: &str = "Qwen3-ASR-1.7B-Q8_0.gguf";
/// Plik projektora dźwięku.
pub const MMPROJ_FILE: &str = "mmproj-Qwen3-ASR-1.7B-Q8_0.gguf";

/// Pozycja katalogu: Qwen3-ASR (ok. 2,5 GB).
pub fn qwen_asr() -> ItemSpec {
    let file = |name: &str, size: u64, sha: &str| FileSpec {
        name: name.into(),
        url: format!("{HF_QWEN_ASR}/{name}"),
        size,
        sha256: Some(sha.into()),
    };
    ItemSpec {
        id: "qwen3-asr-1.7b-q8".into(),
        kind: ModelItemKind::Stt,
        name: "Qwen3-ASR 1.7B (Q8)".into(),
        license: "Apache-2.0".into(),
        source: "https://huggingface.co/ggml-org/Qwen3-ASR-1.7B-GGUF".into(),
        root: Root::Models,
        dir: "qwen3-asr".into(),
        files: vec![
            file(
                MODEL_FILE,
                2_165_034_944,
                "58e22d0532d4eacaf034cfac17a6fed159f37c41390c710186783be439d1fc57",
            ),
            file(
                MMPROJ_FILE,
                355_709_344,
                "46c1d533af3f354ceb37ce855dbceff7da7fa7cf1e6a523df3b13440bd164c0d",
            ),
        ],
        install: Install::Files,
        confirmed: true,
        note: note(
            "Dokładne rozpoznawanie mowy po polsku na karcie graficznej (ok. 0,3–0,5 s na zdanie). Wymaga silnika llama-server z tego katalogu; gdy zawiedzie, rozpoznaje Whisper.",
            "Accurate Polish speech recognition on the GPU (about 0.3–0.5 s per sentence). Requires the llama-server engine from this catalogue; if it fails, Whisper takes over.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_files_are_pinned() {
        let item = qwen_asr();
        assert_eq!(item.files.len(), 2);
        assert!(item.confirmed && item.pinned());
        assert!(item.files.iter().all(|f| f.url.starts_with(HF_QWEN_ASR)));
        assert_eq!(item.files[0].name, MODEL_FILE);
        assert_eq!(item.files[1].name, MMPROJ_FILE);
        let total: u64 = item.files.iter().map(|f| f.size).sum();
        assert!((2_400_000_000..2_700_000_000).contains(&total), "{total}");
    }
}
