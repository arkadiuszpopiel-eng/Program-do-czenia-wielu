//! Supertonic 3 — głos główny (zadanie P1): model z przypiętej wersji oficjalnego archiwum,
//! każdy plik z rozmiarem i sumą SHA-256. Pliki trafiają płasko do `models/supertonic/`
//! (sieci, konfiguracja i barwy głosów F1–F5, M1–M5). Wagi: licencja OpenRAIL-M — jej tekst
//! (`LICENSE` z tej samej rewizji) trafia obok modelu, bo licencja wymaga przekazania kopii
//! (licencje u źródła, C5.1: `docs/vendor/licencje-modeli-i-silnikow.md`).

use app_api::dto::ModelItemKind;

use crate::catalog::{FileSpec, Install, ItemSpec, Root, note};

/// Przypięta wersja repozytorium `supertone-oss-archive/supertonic-3`.
const HF_SUPERTONIC: &str = "https://huggingface.co/supertone-oss-archive/supertonic-3/resolve/aafc6e32416a594460b32413efc49d7fe4ce6d46";

/// (katalog w repozytorium — pusty dla korzenia, plik, rozmiar w bajtach, SHA-256).
const FILES: &[(&str, &str, u64, &str)] = &[
    (
        "",
        "LICENSE",
        15_007,
        "0d944a9110fed9a9602d60e0423a272903e7bd21ab060490774efc77c2275e9f",
    ),
    (
        "onnx",
        "duration_predictor.onnx",
        3_700_147,
        "c3eb91414d5ff8a7a239b7fe9e34e7e2bf8a8140d8375ffb14718b1c639325db",
    ),
    (
        "onnx",
        "text_encoder.onnx",
        36_416_150,
        "c7befd5ea8c3119769e8a6c1486c4edc6a3bc8365c67621c881bbb774b9902ff",
    ),
    (
        "onnx",
        "tts.json",
        8_253,
        "42078d3aef1cd43ab43021f3c54f47d2d75ceb4e75f627f118890128b06a0d09",
    ),
    (
        "onnx",
        "unicode_indexer.json",
        277_676,
        "9bf7346e43883a81f8645c81224f786d43c5b57f3641f6e7671a7d6c493cb24f",
    ),
    (
        "onnx",
        "vector_estimator.onnx",
        256_534_781,
        "883ac868ea0275ef0e991524dc64f16b3c0376efd7c320af6b53f5b780d7c61c",
    ),
    (
        "onnx",
        "vocoder.onnx",
        101_424_195,
        "085de76dd8e8d5836d6ca66826601f615939218f90e519f70ee8a36ed2a4c4ba",
    ),
    (
        "voice_styles",
        "F1.json",
        292_046,
        "bbdec6ee00231c2c742ad05483df5334cab3b52fda3ba38e6a07059c4563dbc2",
    ),
    (
        "voice_styles",
        "F2.json",
        292_423,
        "7c722c6a72707b1a77f035d67f0d1351ba187738e06f7683e8c72b1df3477fc6",
    ),
    (
        "voice_styles",
        "F3.json",
        290_794,
        "12f6ef2573baa2defa1128069cb59f203e3ab67c92af77b42df8a0e3a2f7c6ab",
    ),
    (
        "voice_styles",
        "F4.json",
        291_808,
        "c2fa764c1225a76dfc3e2c73e8aa4f70d9ee48793860eb34c295fff01c2e032b",
    ),
    (
        "voice_styles",
        "F5.json",
        291_479,
        "45966e73316415626cf41a7d1c6f3b4c70dbc1ba2bee5c1978ef0ce33244fc8d",
    ),
    (
        "voice_styles",
        "M1.json",
        291_748,
        "e35604687f5d23694b8e91593a93eec0e4eca6c0b02bb8ed69139ab2ea6b0a5b",
    ),
    (
        "voice_styles",
        "M2.json",
        292_055,
        "b76cbf62bac707c710cf0ae5aba5e31eea1a6339a9734bfae33ab98499534a50",
    ),
    (
        "voice_styles",
        "M3.json",
        290_198,
        "ea1ac35ccb91b0d7ecad533a2fbd0eec10c91513d8951e3b25fbba99954e159b",
    ),
    (
        "voice_styles",
        "M4.json",
        291_522,
        "ca8eefad4fcd989c9379032ff3e50738adc547eeb5e221b82593a6d7b3bac303",
    ),
    (
        "voice_styles",
        "M5.json",
        291_469,
        "dd22b92740314321f8ae11c5e87f8dd60d060f15dd3a632b5adf77f471f77af2",
    ),
];

/// Pozycja katalogu: model Supertonic 3 (ok. 383 MB).
pub fn supertonic() -> ItemSpec {
    ItemSpec {
        id: "supertonic-3".into(),
        kind: ModelItemKind::Tts,
        name: "Supertonic 3 (31 języków, 10 barw)".into(),
        license: "OpenRAIL-M (wagi; lista zakazanych zastosowań), MIT (kod przykładowy)".into(),
        source: "https://huggingface.co/supertone-oss-archive/supertonic-3".into(),
        root: Root::Models,
        dir: "supertonic".into(),
        files: FILES
            .iter()
            .map(|(dir, name, size, sha)| FileSpec {
                name: (*name).into(),
                url: if dir.is_empty() {
                    format!("{HF_SUPERTONIC}/{name}")
                } else {
                    format!("{HF_SUPERTONIC}/{dir}/{name}")
                },
                size: *size,
                sha256: Some((*sha).into()),
            })
            .collect(),
        install: Install::Files,
        confirmed: true,
        targets: Vec::new(),
        note: note(
            "Naturalny głos po polsku, działa na procesorze (ok. 1 s na zdanie). Cztery agentki mówią czterema różnymi barwami. Wymaga sidecara alfa-supertonic (jest w programie).",
            "Natural Polish voice, runs on the CPU (about 1 s per sentence). The four agents speak with four different timbres. Requires the alfa-supertonic sidecar (shipped with the app).",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_file_is_pinned_and_flat() {
        let item = supertonic();
        assert_eq!(item.files.len(), 17);
        // Tekst licencji wag (Open RAIL-M wymaga przekazania kopii) z tej samej rewizji.
        assert!(
            item.files
                .iter()
                .any(|f| f.name == "LICENSE" && f.url == format!("{HF_SUPERTONIC}/LICENSE"))
        );
        assert!(item.confirmed);
        for f in &item.files {
            assert!(f.url.starts_with(HF_SUPERTONIC), "{}", f.url);
            assert!(
                !f.name.contains('/') && !f.name.contains('\\'),
                "{}",
                f.name
            );
            assert_eq!(f.sha256.as_deref().map(str::len), Some(64));
            assert!(f.size > 0);
        }
        let total: u64 = item.files.iter().map(|f| f.size).sum();
        assert!((380_000_000..420_000_000).contains(&total), "{total}");
        for voice in ["F1", "F2", "F4", "F5"] {
            assert!(item.files.iter().any(|f| f.name == format!("{voice}.json")));
        }
    }
}
