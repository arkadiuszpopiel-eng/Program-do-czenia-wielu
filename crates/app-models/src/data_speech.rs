//! Pozycje mowy w katalogu (Whisper, Piper, Silero VAD, słowo wywoławcze) — wydzielone z
//! `data.rs` (limit 400 linii na plik). Zasady potwierdzania jak w `data.rs`.

use app_api::dto::ModelItemKind;

use super::item;
use crate::catalog::{FileSpec, Install, ItemSpec, Pick, Root};

/// Przypięta rewizja `ggerganov/whisper.cpp`; sumy SHA-256 plików z API Hugging Face (LFS oid,
/// 2026-10-21 — licencje u źródła, C5.1).
const HF_WHISPER: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1";
/// Tag v1.0.0 repozytorium `rhasspy/piper-voices` przypięty do jego commitu (2026-10-21).
const HF_PIPER: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/375a0fe641dea077c2a47b4e9a056d6da521eed3/pl/pl_PL/gosia/medium";

fn whisper(id: &str, file: &str, mib: u64, sha256: &str, (pl, en): (&str, &str)) -> ItemSpec {
    item(
        id,
        ModelItemKind::Stt,
        &format!(
            "Whisper {}",
            file.trim_start_matches("ggml-").trim_end_matches(".bin")
        ),
        "MIT",
        "https://huggingface.co/ggerganov/whisper.cpp",
        (Root::Models, "whisper"),
        vec![FileSpec::new(
            file,
            &format!("{HF_WHISPER}/{file}"),
            mib,
            Some(sha256),
        )],
        Install::Files,
        false,
        (pl, en),
    )
}

fn pypi(member: &str, dest: &str, sha: &str) -> Pick {
    Pick {
        member: member.into(),
        dest: dest.into(),
        sha256: Some(sha.into()),
    }
}

pub(super) fn speech() -> Vec<ItemSpec> {
    let speaker_template = serde_json::json!({
        "format": "alfa-speaker-v1", "name": "wespeaker_en_voxceleb_resnet34",
        "license": "Apache-2.0", "path": "wespeaker_en_voxceleb_resnet34.onnx", "sha256": "",
        "n_mels": 80, "frames": 200, "hop_frames": 100, "cmn": true
    });
    vec![
        whisper(
            "whisper-large-v3-turbo-q5_0",
            "ggml-large-v3-turbo-q5_0.bin",
            547,
            "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
            ("Rozpoznawanie mowy (profil A, GPU/CPU). Wymaga sidecara whisper-server.",
             "Speech recognition (profile A, GPU/CPU). Requires the whisper-server sidecar."),
        ),
        whisper(
            "whisper-small-q5_1",
            "ggml-small-q5_1.bin",
            181,
            "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
            ("Lżejszy model STT na CPU (zapas). Aplikacja bierze pierwszy model ggml-* alfabetycznie.",
             "Lighter CPU speech model (fallback). The app uses the first ggml-* model alphabetically."),
        ),
        crate::qwen_asr::qwen_asr(),
        crate::supertonic::supertonic(),
        crate::voice_runtime::smart_turn(),
        crate::voice_runtime::onnxruntime(),
        crate::voice_runtime::onnxruntime_directml(),
        crate::voice_runtime::directml(),
        item(
            "piper-pl_PL-gosia-medium",
            ModelItemKind::Tts,
            "Piper pl_PL gosia (medium)",
            "MIT (głos: do potwierdzenia — MODEL_CARD)",
            "https://huggingface.co/rhasspy/piper-voices",
            (Root::Models, "piper"),
            vec![
                FileSpec::new("pl_PL-gosia-medium.onnx", &format!("{HF_PIPER}/pl_PL-gosia-medium.onnx"), 61, Some("38f66464240ed74f186e6b7dc13c6e3b22e023426299f25c2b3cc9dfa9373fbc")),
                FileSpec::new("pl_PL-gosia-medium.onnx.json", &format!("{HF_PIPER}/pl_PL-gosia-medium.onnx.json"), 0, Some("1aefb31a9d53ffe44a8163ff73ec833acb7a6253848f6bb0403d8a66f9c7510d")),
            ],
            Install::Files,
            false,
            ("Zapasowy głos TTS (Piper, proces na zdanie). Wymaga sidecara piper.",
             "Fallback TTS voice (Piper, process per sentence). Requires the piper sidecar."),
        ),
        item(
            "silero-vad",
            ModelItemKind::Vad,
            "Silero VAD 6.2.3 (op18, bez If)",
            "MIT",
            "https://pypi.org/project/silero-vad/6.2.3/",
            (Root::Models, "silero"),
            vec![FileSpec {
                name: "silero_vad-6.2.3-py3-none-any.whl".into(),
                url: "https://files.pythonhosted.org/packages/84/ef/9099037ed6f180ea33220178df4107112c0ce2bf5fb4d6f6ab19db2844ed/silero_vad-6.2.3-py3-none-any.whl".into(),
                size: 11_317_527,
                sha256: Some("7b7f5436cfcb02fae583a05b512ea96467fd449fe54cb49a5e4f06c51a1e43b8".into()),
            }],
            Install::Pick(vec![pypi(
                "silero_vad/data/silero_vad_op18_ifless.onnx",
                "silero_vad.onnx",
                "7671cd04b004e9076da0d4a7b1a5aec36adf161c39230c1cb94a4fd5db6bbd28",
            )]),
            true,
            ("Wykrywanie mowy (bez modelu — detektor energii). Z paczki PyPI, hash przypięty.",
             "Voice activity detection (without it — energy detector). From the PyPI package, pinned hash."),
        ),
        item(
            "openwakeword-features",
            ModelItemKind::Wake,
            "openWakeWord 0.5.1: melspektrogram + embedding",
            "Apache-2.0 (embedding: Google speech_embedding — sprawdzić przed dystrybucją)",
            "https://pypi.org/project/openwakeword/0.5.1/",
            (Root::Models, "kws"),
            vec![FileSpec {
                name: "openwakeword-0.5.1-py3-none-any.whl".into(),
                url: "https://files.pythonhosted.org/packages/00/b7/4bbef6bd840866579672eb7240c19f547dcb12dc685dbf725a67a6afbac3/openwakeword-0.5.1-py3-none-any.whl".into(),
                size: 16_669_337,
                sha256: Some("015467e49f08b0ef8efa433690ce70566abb72f7c36501a6fa893762b4ddc559".into()),
            }],
            Install::Pick(vec![
                pypi("openwakeword/resources/models/melspectrogram.onnx", "melspectrogram.onnx",
                     "ba2b0e0f8b7b875369a2c89cb13360ff53bac436f2895cced9f479fa65eb176f"),
                pypi("openwakeword/resources/models/embedding_model.onnx", "embedding_model.onnx",
                     "70d164290c1d095d1d4ee149bc5e00543250a7316b59f31d056cff7bd3075c1f"),
            ]),
            true,
            ("Cechy słów wywoławczych. Klasyfikator „Hej Alfa” (PL) i manifest *.kws.json — własny trening (bramka #3).",
             "Wake-word features. The “Hej Alfa” (PL) classifier and *.kws.json manifest need own training (gate #3)."),
        ),
        item(
            "wespeaker-resnet34",
            ModelItemKind::Speaker,
            "WeSpeaker ResNet34 (VoxCeleb)",
            "Apache-2.0 (dane VoxCeleb — sprawdzić warunki)",
            "https://github.com/k2-fsa/sherpa-onnx/releases/tag/speaker-recongition-models",
            (Root::Models, "speaker"),
            vec![FileSpec::new(
                "wespeaker_en_voxceleb_resnet34.onnx",
                "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/wespeaker_en_voxceleb_resnet34.onnx",
                26,
                // Zasób wydania z 2024 bez digestu w API — suma z pobranego pliku (26 534 365 B).
                Some("5ef208a9da1453335308a6b6f4e6dfbd7e183a38b604de0a57664f45d257fe94"),
            )],
            Install::Speaker { manifest: "wespeaker-resnet34.speaker.json".into(), template: speaker_template },
            false,
            ("Weryfikacja właściciela (embedding mówcy). Okno 200 ramek — do potwierdzenia pomiarem EER.",
             "Owner verification (speaker embedding). 200-frame window — to be confirmed by EER measurement."),
        ),
    ]
}
