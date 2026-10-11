//! Pozycje katalogu dla rozmowy (zadanie P1): model końca wypowiedzi Smart Turn v3.2 i biblioteka
//! ONNX Runtime dla procesów pobocznych mowy (`alfa-supertonic`, `alfa-smart-turn`). Rozmiary i sumy
//! SHA-256 z list plików wydawców, sprawdzone na pobranych plikach 2026-10-11.

use app_api::dto::ModelItemKind;

use crate::catalog::{FileSpec, Install, ItemSpec, Pick, Root, note};

/// Przypięta wersja repozytorium `pipecat-ai/smart-turn-v3`.
const HF_SMART_TURN: &str = "https://huggingface.co/pipecat-ai/smart-turn-v3/resolve/f766f81d3cfdf7737ac64aad813d91bbfd56bf93";

/// Plik modelu Smart Turn.
pub const SMART_TURN_FILE: &str = "smart-turn-v3.2-cpu.onnx";
/// Katalog modelu w `models/`.
pub const SMART_TURN_DIR: &str = "turn";
/// Wersja ONNX Runtime.
pub const ORT_VERSION: &str = "1.23.1";
/// Katalog biblioteki w `sidecars/`.
pub const ORT_DIR: &str = "onnxruntime";
/// Plik biblioteki.
pub const ORT_FILE: &str = "onnxruntime.dll";

/// Pozycja katalogu: Smart Turn v3.2 (ok. 9 MB).
pub fn smart_turn() -> ItemSpec {
    ItemSpec {
        id: "smart-turn-v3.2".into(),
        kind: ModelItemKind::Vad,
        name: "Smart Turn v3.2 (koniec wypowiedzi)".into(),
        license: "BSD-2-Clause".into(),
        source: "https://huggingface.co/pipecat-ai/smart-turn-v3".into(),
        root: Root::Models,
        dir: SMART_TURN_DIR.into(),
        files: vec![FileSpec {
            name: SMART_TURN_FILE.into(),
            url: format!("{HF_SMART_TURN}/{SMART_TURN_FILE}"),
            size: 8_679_182,
            sha256: Some("2bb026316b14a660486a75b1733cd3fbab8c2fd0314dc9af7be49f8cca967e4f".into()),
        }],
        install: Install::Files,
        confirmed: true,
        note: note(
            "Rozpoznaje po brzmieniu, czy skończyłeś mówić — Alfa nie wchodzi w słowo w połowie zdania i nie czeka niepotrzebnie po jego końcu. Liczy na procesorze (ok. 25 ms). Wymaga biblioteki ONNX Runtime z tego katalogu.",
            "Tells from the sound whether you have finished speaking — Alfa does not cut in mid-sentence and does not wait needlessly after it. Runs on the CPU (about 25 ms). Requires the ONNX Runtime library from this catalogue.",
        ),
    }
}

/// Pozycja katalogu: ONNX Runtime (procesor) dla procesów pobocznych mowy.
pub fn onnxruntime() -> ItemSpec {
    let folder = format!("onnxruntime-win-x64-{ORT_VERSION}");
    ItemSpec {
        id: "onnxruntime-cpu".into(),
        kind: ModelItemKind::Sidecar,
        name: format!("ONNX Runtime {ORT_VERSION} (procesor)"),
        license: "MIT".into(),
        source: "https://github.com/microsoft/onnxruntime/releases".into(),
        root: Root::Sidecars,
        dir: ORT_DIR.into(),
        files: vec![FileSpec {
            name: format!("{folder}.zip"),
            url: format!(
                "https://github.com/microsoft/onnxruntime/releases/download/v{ORT_VERSION}/{folder}.zip"
            ),
            size: 78_099_392,
            sha256: Some("d57817ee1e1aec8a7b4973ded0536f9be19ce0aa2152ff06a07952ed44790ed7".into()),
        }],
        install: Install::Pick(vec![
            Pick {
                member: format!("{folder}/lib/{ORT_FILE}"),
                dest: ORT_FILE.into(),
                sha256: Some(
                    "a59e26d1eec191ce049a9754a5f7e53bfe095c1b2a9276c7510e33f1fb76c84c".into(),
                ),
            },
            Pick {
                member: format!("{folder}/LICENSE"),
                dest: "LICENSE".into(),
                sha256: Some(
                    "c250d6278f0b47a6439fb7592b08b58a55eb9f535aa49a1db63211c3f982b674".into(),
                ),
            },
            Pick {
                member: format!("{folder}/ThirdPartyNotices.txt"),
                dest: "ThirdPartyNotices.txt".into(),
                sha256: Some(
                    "e00f828e0a33de591a355ae6606d2625f5758da7d2c844db7821c9dd3e3647b6".into(),
                ),
            },
        ]),
        confirmed: true,
        note: note(
            "Biblioteka obliczeń dla głosu Supertonic i dla wykrywania końca wypowiedzi. Działa tylko w osobnych procesach pobocznych, nie w samym programie.",
            "Compute library for the Supertonic voice and end-of-turn detection. Runs only in separate helper processes, not in the app itself.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_turn_is_pinned() {
        let item = smart_turn();
        assert!(item.confirmed && item.pinned());
        assert_eq!(item.files.len(), 1);
        assert!(item.files[0].url.starts_with(HF_SMART_TURN));
        assert!(item.files[0].url.ends_with(SMART_TURN_FILE));
        assert_eq!(item.dir, SMART_TURN_DIR);
    }

    #[test]
    fn runtime_archive_and_library_are_pinned() {
        let item = onnxruntime();
        assert!(item.confirmed && item.pinned());
        assert!(item.files[0].url.contains(ORT_VERSION));
        let Install::Pick(picks) = &item.install else {
            panic!("biblioteka ma być wybierana z archiwum");
        };
        assert_eq!(picks[0].dest, ORT_FILE);
        assert!(picks[0].member.ends_with("/lib/onnxruntime.dll"));
        assert!(picks[0].sha256.is_some());
        assert_eq!(picks.len(), 3);
    }
}
