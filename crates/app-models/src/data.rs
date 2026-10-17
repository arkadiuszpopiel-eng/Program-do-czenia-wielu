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

use crate::catalog::{FileSpec, Install, ItemSpec, Root, exe, note};

#[path = "data_speech.rs"]
mod speech_items;
use speech_items::speech;

/// Wydanie llama.cpp dla `llama-server` (do przypięcia przez człowieka).
pub const LLAMA_TAG: &str = "b11559";

/// Archiwa wydania [`LLAMA_TAG`]: (rozmiar w bajtach, SHA-256) z listy plików wydania na GitHubie
/// (2026-10-11); suma archiwum CUDA sprawdzona dodatkowo po pobraniu.
const LLAMA_CPU_ZIP: (u64, &str) = (
    19_501_338,
    "1228f133868c02a424bcf663a688e66d8c7d5499077649f305188f756f584377",
);
const LLAMA_VULKAN_ZIP: (u64, &str) = (
    32_084_119,
    "bf2fb2a7fc5d7669ccabbb6de3f6dd6b54058f213b32e731abc62b590f8566dc",
);
const LLAMA_CUDA_ZIP: (u64, &str) = (
    265_207_128,
    "4a885b80b26a213e86b774a580ac14fb210eba6220579051339c71b2bda1ecd7",
);
const CUDART_ZIP: (u64, &str) = (
    391_443_627,
    "8c79a9b226de4b3cacfd1f83d24f962d0773be79f1e7b75c6af4ded7e32ae1d6",
);
/// Wydanie whisper.cpp dla `whisper-server` (SPEC `voice-stt`: ≥ 1.8.1, przypięte).
pub const WHISPER_TAG: &str = "v1.8.1";

#[allow(clippy::too_many_arguments)]
pub(super) fn item(
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

fn llama(backend: &str, asset: &str, (size, sha256): (u64, &str)) -> ItemSpec {
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
        vec![FileSpec {
            name: format!("llama-{LLAMA_TAG}-{asset}.zip"),
            url,
            size,
            sha256: Some(sha256.into()),
        }],
        Install::Tree {
            strip: String::new(),
            require: vec![exe("llama-server")],
        },
        true,
        (
            "Serwer modeli lokalnych (127.0.0.1, losowy port i klucz). Suma archiwum przypięta; to wydanie obsługuje też rozpoznawanie mowy Qwen3-ASR.",
            "Local model server (127.0.0.1, random port and key). Archive checksum pinned; this release also runs Qwen3-ASR speech recognition.",
        ),
    )
}

/// Wariant CUDA wydań llama.cpp dla Windows (laptop z RTX 4050; do potwierdzenia przez człowieka).
const LLAMA_CUDA: &str = "cuda-12.4";

/// `llama-server` CUDA: archiwum serwera i osobne archiwum bibliotek `cudart`/cuBLAS z tego
/// samego wydania, oba rozpakowywane do `sidecars/llama-cuda/` (wymagany też `cudart64_12.dll`
/// — bez niego serwer po cichu liczyłby na CPU). Bez tej pozycji profil CUDA używa kompilacji
/// Vulkan albo CPU (`app-modules::route::local::server_for`, ostrzeżenie w dzienniku).
fn llama_cuda() -> ItemSpec {
    let base = format!("https://github.com/ggml-org/llama.cpp/releases/download/{LLAMA_TAG}");
    item(
        "sidecar-llama-cuda",
        ModelItemKind::Sidecar,
        &format!("llama-server (cuda, llama.cpp {LLAMA_TAG})"),
        "MIT; cudart/cuBLAS — NVIDIA CUDA EULA (redystrybucja do potwierdzenia)",
        "https://github.com/ggml-org/llama.cpp/releases",
        (Root::Sidecars, "llama-cuda"),
        vec![
            FileSpec {
                name: format!("llama-{LLAMA_TAG}-{LLAMA_CUDA}.zip"),
                url: format!("{base}/llama-{LLAMA_TAG}-bin-win-{LLAMA_CUDA}-x64.zip"),
                size: LLAMA_CUDA_ZIP.0,
                sha256: Some(LLAMA_CUDA_ZIP.1.into()),
            },
            FileSpec {
                name: format!("cudart-{LLAMA_CUDA}.zip"),
                url: format!("{base}/cudart-llama-bin-win-{LLAMA_CUDA}-x64.zip"),
                size: CUDART_ZIP.0,
                sha256: Some(CUDART_ZIP.1.into()),
            },
        ],
        Install::Tree {
            strip: String::new(),
            require: vec![exe("llama-server"), "cudart64_12.dll".into()],
        },
        false,
        (
            "Serwer modeli lokalnych na kartach NVIDIA (CUDA 12.4, z bibliotekami cudart). Bez niego profil CUDA używa kompilacji Vulkan albo CPU (ostrzeżenie w dzienniku). Sumy archiwów przypięte; licencja bibliotek NVIDIA do potwierdzenia przed wydaniem publicznym.",
            "Local model server for NVIDIA GPUs (CUDA 12.4, with cudart libraries). Without it the CUDA profile uses the Vulkan or CPU build (warning in the log). Archive checksums pinned; the NVIDIA library licence is to be confirmed before a public release.",
        ),
    )
}

/// `whisper-server` CUDA 12.4 (karta NVIDIA; PLAN §6.3 D-CUDA): archiwum wydania zawiera też
/// `cudart`/cuBLAS (układ `Release/…` sprawdzony 2026-10-06 listą wpisów archiwum: 28 plików,
/// ~0,7 GB po rozpakowaniu). Zainstalowana — ma pierwszeństwo przed CPU (`app_modules::stt`).
fn whisper_cuda() -> ItemSpec {
    item(
        "sidecar-whisper-cuda",
        ModelItemKind::Sidecar,
        &format!("whisper-server (CUDA 12.4, whisper.cpp {WHISPER_TAG})"),
        "MIT; cudart/cuBLAS — NVIDIA CUDA EULA (redystrybucja do potwierdzenia)",
        "https://github.com/ggml-org/whisper.cpp/releases",
        (Root::Sidecars, "whisper-cuda"),
        vec![FileSpec::new(
            "whisper-cublas-12.4.0-bin-x64.zip",
            &format!(
                "https://github.com/ggml-org/whisper.cpp/releases/download/{WHISPER_TAG}/whisper-cublas-12.4.0-bin-x64.zip"
            ),
            430,
            None,
        )],
        Install::Tree {
            strip: "Release/".into(),
            require: vec![exe("whisper-server"), "cudart64_12.dll".into()],
        },
        false,
        (
            "Rozpoznawanie mowy na karcie NVIDIA (CUDA 12.4, z bibliotekami cudart). Wymaga sterownika NVIDIA; bez niego Alfa używa wersji CPU.",
            "Speech recognition on NVIDIA GPUs (CUDA 12.4, with cudart libraries). Needs the NVIDIA driver; without it Alfa uses the CPU build.",
        ),
    )
}

fn sidecars() -> Vec<ItemSpec> {
    vec![
        llama("vulkan", "vulkan", LLAMA_VULKAN_ZIP),
        llama_cuda(),
        llama("cpu", "cpu", LLAMA_CPU_ZIP),
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
                "Serwer STT (127.0.0.1), wymagany także przy wersji CUDA (zapas). Wersji Vulkan nie ma w wydaniach (sprawdzone do v1.9.5, 2026-10-17) — na kartach AMD i Intel mowę rozpoznaje Qwen3-ASR.",
                "STT server (127.0.0.1), required also with the CUDA build (fallback). There is no Vulkan build in the releases (checked up to v1.9.5, 2026-10-17) — on AMD and Intel GPUs Qwen3-ASR recognises speech.",
            ),
        ),
        whisper_cuda(),
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
        // `tools-media`: konwersje (`media_convert`, odtwarzanie innych formatów niż WAV) — bez
        // pobierania automatycznego; build i licencja do potwierdzenia przez człowieka.
        item(
            "sidecar-ffmpeg",
            ModelItemKind::Sidecar,
            "ffmpeg (konwersje multimediów)",
            "LGPL-2.1+ albo GPL-2.0+ (zależnie od buildu)",
            "https://ffmpeg.org/download.html",
            (Root::Sidecars, "ffmpeg"),
            Vec::new(),
            Install::Manual(vec![exe("ffmpeg")]),
            false,
            (
                "Instalacja ręczna: skopiuj ffmpeg.exe (build do potwierdzenia) do sidecars/ffmpeg.",
                "Manual install: copy ffmpeg.exe (build to be confirmed) into sidecars/ffmpeg.",
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
