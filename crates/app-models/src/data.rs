//! Pozycje głosu i sidecarów (dane katalogu).
//!
//! **Potwierdzone** (`confirmed: true`): paczki PyPI pobrane i sprawdzone przy tworzeniu katalogu
//! (2026-10-04) — adres, rozmiar i SHA-256 archiwum z API PyPI, SHA-256 wybranych wpisów policzony
//! lokalnie (Silero VAD = `voice_vad_impl::KNOWN_MODELS`).
//!
//! **Do potwierdzenia przez człowieka** (`confirmed: false`, bez przypiętego SHA-256): HuggingFace
//! i GitHub były zablokowane z sesji tworzącej katalog — adresy, rozmiary, układ archiwów i licencje
//! pochodzą z wiedzy o repozytoriach. Przed wydaniem człowiek sprawdza adres, wpisuje SHA-256
//! (`sha256sum`) i zmienia `confirmed`; do tego czasu UI wymaga zgody TOFU z policzonym hashem.

use app_api::dto::ModelItemKind;

use crate::catalog::{FileSpec, Install, ItemSpec, Pick, Root, exe, note};

/// Wydanie llama.cpp dla `llama-server` (do przypięcia przez człowieka).
pub const LLAMA_TAG: &str = "b6710";
/// Wydanie whisper.cpp dla `whisper-server` (SPEC `voice-stt`: ≥ 1.8.1, przypięte).
pub const WHISPER_TAG: &str = "v1.8.1";

const HF_WHISPER: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
const HF_PIPER: &str =
    "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/pl/pl_PL/gosia/medium";

#[allow(clippy::too_many_arguments)]
fn item(
    id: &str,
    kind: ModelItemKind,
    name: &str,
    license: &str,
    source: &str,
    (root, dir): (Root, &str),
    files: Vec<FileSpec>,
    install: Install,
    confirmed: bool,
    (pl, en): (&str, &str),
) -> ItemSpec {
    ItemSpec {
        id: id.into(),
        kind,
        name: name.into(),
        license: license.into(),
        source: source.into(),
        root,
        dir: dir.into(),
        files,
        install,
        confirmed,
        note: note(pl, en),
    }
}

fn whisper(id: &str, file: &str, mib: u64, (pl, en): (&str, &str)) -> ItemSpec {
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
            None,
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

fn speech() -> Vec<ItemSpec> {
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
            ("Rozpoznawanie mowy (profil A, GPU/CPU). Wymaga sidecara whisper-server.",
             "Speech recognition (profile A, GPU/CPU). Requires the whisper-server sidecar."),
        ),
        whisper(
            "whisper-small-q5_1",
            "ggml-small-q5_1.bin",
            181,
            ("Lżejszy model STT na CPU (zapas). Aplikacja bierze pierwszy model ggml-* alfabetycznie.",
             "Lighter CPU speech model (fallback). The app uses the first ggml-* model alphabetically."),
        ),
        item(
            "piper-pl_PL-gosia-medium",
            ModelItemKind::Tts,
            "Piper pl_PL gosia (medium)",
            "MIT (głos: do potwierdzenia — MODEL_CARD)",
            "https://huggingface.co/rhasspy/piper-voices",
            (Root::Models, "piper"),
            vec![
                FileSpec::new("pl_PL-gosia-medium.onnx", &format!("{HF_PIPER}/pl_PL-gosia-medium.onnx"), 61, None),
                FileSpec::new("pl_PL-gosia-medium.onnx.json", &format!("{HF_PIPER}/pl_PL-gosia-medium.onnx.json"), 0, None),
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
                None,
            )],
            Install::Speaker { manifest: "wespeaker-resnet34.speaker.json".into(), template: speaker_template },
            false,
            ("Weryfikacja właściciela (embedding mówcy). Okno 200 ramek — do potwierdzenia pomiarem EER.",
             "Owner verification (speaker embedding). 200-frame window — to be confirmed by EER measurement."),
        ),
    ]
}

fn llama(backend: &str, asset: &str) -> ItemSpec {
    let url = format!(
        "https://github.com/ggml-org/llama.cpp/releases/download/{LLAMA_TAG}/llama-{LLAMA_TAG}-bin-win-{asset}-x64.zip"
    );
    item(
        &format!("sidecar-llama-{backend}"),
        ModelItemKind::Sidecar,
        &format!("llama-server ({backend}, llama.cpp {LLAMA_TAG})"),
        "MIT",
        "https://github.com/ggml-org/llama.cpp/releases",
        (Root::Sidecars, &format!("llama-{backend}")),
        vec![FileSpec::new(
            &format!("llama-{LLAMA_TAG}-{asset}.zip"),
            &url,
            40,
            None,
        )],
        Install::Tree {
            strip: String::new(),
            require: vec![exe("llama-server")],
        },
        false,
        (
            "Serwer modeli lokalnych (127.0.0.1, losowy port i klucz). Wersja i układ archiwum do potwierdzenia.",
            "Local model server (127.0.0.1, random port and key). Version and archive layout to be confirmed.",
        ),
    )
}

fn sidecars() -> Vec<ItemSpec> {
    vec![
        llama("vulkan", "vulkan"),
        llama("cpu", "cpu"),
        item(
            "sidecar-whisper-cpu",
            ModelItemKind::Sidecar,
            &format!("whisper-server (CPU, whisper.cpp {WHISPER_TAG})"),
            "MIT",
            "https://github.com/ggml-org/whisper.cpp/releases",
            (Root::Sidecars, "whisper"),
            vec![FileSpec::new(
                "whisper-bin-x64.zip",
                &format!(
                    "https://github.com/ggml-org/whisper.cpp/releases/download/{WHISPER_TAG}/whisper-bin-x64.zip"
                ),
                8,
                None,
            )],
            Install::Tree {
                strip: "Release/".into(),
                require: vec![exe("whisper-server")],
            },
            false,
            (
                "Serwer STT (127.0.0.1). Buildy Vulkan/CUDA — osobno (do potwierdzenia).",
                "STT server (127.0.0.1). Vulkan/CUDA builds — separately (to be confirmed).",
            ),
        ),
        item(
            "sidecar-piper",
            ModelItemKind::Sidecar,
            "piper (2023.11.14-2)",
            "MIT",
            "https://github.com/rhasspy/piper/releases/tag/2023.11.14-2",
            (Root::Sidecars, "piper"),
            vec![FileSpec::new(
                "piper_windows_amd64.zip",
                "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip",
                22,
                None,
            )],
            Install::Tree {
                strip: "piper/".into(),
                require: vec![exe("piper")],
            },
            false,
            (
                "Silnik TTS Piper (z espeak-ng-data). Repozytorium zarchiwizowane — następca OHF-Voice/piper1-gpl (GPL).",
                "Piper TTS engine (with espeak-ng-data). Repository archived — successor OHF-Voice/piper1-gpl (GPL).",
            ),
        ),
        item(
            "sidecar-pocket-tts",
            ModelItemKind::Sidecar,
            "Pocket TTS PL (wrapper JSON-lines)",
            "CC-BY-4.0 (model); wrapper — własny",
            "crates/voice-tts-impl/README.md",
            (Root::Sidecars, "pocket-tts"),
            Vec::new(),
            Install::Manual(vec![exe("pocket-tts")]),
            false,
            (
                "Instalacja ręczna: wrapper budowany osobno (ADR 11), modele w models/pocket-tts.",
                "Manual install: the wrapper is built separately (ADR 11), models in models/pocket-tts.",
            ),
        ),
    ]
}

/// Pozycje głosu (STT, TTS, VAD, KWS, mówca) i sidecarów.
pub fn voice_and_sidecars() -> Vec<ItemSpec> {
    let mut out = speech();
    out.extend(sidecars());
    out
}
