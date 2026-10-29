//! Pozycje katalogu dla rozmowy (zadanie P1): model końca wypowiedzi Smart Turn v3.2 i biblioteka
//! ONNX Runtime dla procesów pobocznych mowy (`alfa-supertonic`, `alfa-smart-turn`). Rozmiary i sumy
//! SHA-256 z list plików wydawców, sprawdzone na pobranych plikach 2026-10-11.

use app_api::dto::ModelItemKind;

use crate::catalog::Target;
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
        targets: Vec::new(),
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
        targets: vec![Target::WINDOWS_X64],
        note: note(
            "Biblioteka obliczeń dla głosu Supertonic i dla wykrywania końca wypowiedzi. Działa tylko w osobnych procesach pobocznych, nie w samym programie.",
            "Compute library for the Supertonic voice and end-of-turn detection. Runs only in separate helper processes, not in the app itself.",
        ),
    }
}

/// Wersja ONNX Runtime z DirectML (NuGet `Microsoft.ML.OnnxRuntime.DirectML`; wariantu 1.23.1 nie
/// wydano). `ort` 2.0.0-rc.10 wymaga API ≥ 1.22 — obie wersje pasują.
pub const ORT_DML_VERSION: &str = "1.23.0";
/// Wersja DirectML wymagana przez ten pakiet ONNX Runtime (zależność w `.nuspec`).
pub const DIRECTML_VERSION: &str = "1.15.4";
/// Katalog bibliotek DirectML w `sidecars/` — obie pozycje (ONNX Runtime i DirectML) w jednym
/// katalogu: `onnxruntime.dll` ładuje `DirectML.dll` ze swojego katalogu (sprawdzone 2026-10-17
/// listą modułów procesu `alfa-supertonic`).
pub const ORT_DML_DIR: &str = "onnxruntime-dml";
/// Plik biblioteki DirectML.
pub const DIRECTML_FILE: &str = "DirectML.dll";

const NUGET: &str = "https://api.nuget.org/v3-flatcontainer";

/// Pozycja katalogu: ONNX Runtime z DirectML — głos Supertonic na karcie graficznej każdego
/// producenta (DirectX 12). Sumy: pakiet zgodny z `packageHash` (SHA-512) katalogu NuGet,
/// SHA-256 policzone na pobranych plikach 2026-10-17.
pub fn onnxruntime_directml() -> ItemSpec {
    let pkg = format!("microsoft.ml.onnxruntime.directml.{ORT_DML_VERSION}.nupkg");
    ItemSpec {
        id: "onnxruntime-directml".into(),
        kind: ModelItemKind::Sidecar,
        name: format!("ONNX Runtime {ORT_DML_VERSION} (DirectML — karta graficzna)"),
        license: "MIT".into(),
        source: "https://www.nuget.org/packages/Microsoft.ML.OnnxRuntime.DirectML".into(),
        root: Root::Sidecars,
        dir: ORT_DML_DIR.into(),
        files: vec![FileSpec {
            name: pkg.clone(),
            url: format!("{NUGET}/microsoft.ml.onnxruntime.directml/{ORT_DML_VERSION}/{pkg}"),
            size: 12_746_067,
            sha256: Some("a33ec2382b3c440bab74042a135733bb6e5085f293b908d3997688a58fe307e7".into()),
        }],
        install: Install::Pick(vec![
            Pick {
                member: format!("runtimes/win-x64/native/{ORT_FILE}"),
                dest: ORT_FILE.into(),
                sha256: Some(
                    "f5131591edac6b0a8090d0e329040a49319d7a689cb5b465235fbf7030fa8027".into(),
                ),
            },
            Pick {
                member: "LICENSE".into(),
                dest: "LICENSE".into(),
                sha256: Some(
                    "c250d6278f0b47a6439fb7592b08b58a55eb9f535aa49a1db63211c3f982b674".into(),
                ),
            },
            Pick {
                member: "ThirdPartyNotices.txt".into(),
                dest: "ThirdPartyNotices.txt".into(),
                sha256: Some(
                    "e00f828e0a33de591a355ae6606d2625f5758da7d2c844db7821c9dd3e3647b6".into(),
                ),
            },
        ]),
        confirmed: true,
        targets: vec![Target::WINDOWS_X64],
        note: note(
            "Biblioteka obliczeń, która pozwala głosowi Supertonic liczyć na karcie graficznej (NVIDIA, AMD, Intel) przez DirectML. Wymaga też pozycji „DirectML”. Na RX 9070 XT zdanie powstaje w ok. 0,6 s zamiast ok. 1 s na procesorze; zajmuje ok. 540 MB pamięci karty, dlatego Alfa używa jej sama tylko na karcie z własną pamięcią ≥ 12 GB (osobnej albo zintegrowanej z taką rezerwą w BIOS-ie).",
            "Compute library that lets the Supertonic voice run on the graphics card (NVIDIA, AMD, Intel) through DirectML. Also needs the “DirectML” item. On an RX 9070 XT a sentence takes about 0.6 s instead of about 1 s on the CPU; it uses about 540 MB of GPU memory, so Alfa uses it automatically only on a card with its own ≥ 12 GB of memory (discrete, or integrated with such a BIOS reservation).",
        ),
    }
}

/// Pozycja katalogu: DirectML (biblioteka Microsoftu dla ONNX Runtime z DirectML). Pakiet NuGet
/// ma 202 MB, z czego Alfa zostawia tylko bibliotekę x64 (18,5 MB) i licencje.
pub fn directml() -> ItemSpec {
    let pkg = format!("microsoft.ai.directml.{DIRECTML_VERSION}.nupkg");
    ItemSpec {
        id: "directml".into(),
        kind: ModelItemKind::Sidecar,
        name: format!("DirectML {DIRECTML_VERSION}"),
        license: "Microsoft Software License Terms — DirectML (redystrybucja w aplikacjach dozwolona, §1a; zbieranie danych diagnostycznych, §2a)".into(),
        source: "https://www.nuget.org/packages/Microsoft.AI.DirectML".into(),
        root: Root::Sidecars,
        dir: ORT_DML_DIR.into(),
        files: vec![FileSpec {
            name: pkg.clone(),
            url: format!("{NUGET}/microsoft.ai.directml/{DIRECTML_VERSION}/{pkg}"),
            size: 202_292_617,
            sha256: Some("4e7cb7ddce8cf837a7a75dc029209b520ca0101470fcdf275c1f49736a3615b9".into()),
        }],
        install: Install::Pick(vec![
            Pick {
                member: format!("bin/x64-win/{DIRECTML_FILE}"),
                dest: DIRECTML_FILE.into(),
                sha256: Some(
                    "9c9e6d822561c6c41b90e6994b3e8857cf1d66dbfb1e0c4c799c7c89b4e92da1".into(),
                ),
            },
            Pick {
                member: "LICENSE.txt".into(),
                dest: "LICENSE-DirectML.txt".into(),
                sha256: Some(
                    "a05138e3a085ff60a44881eedfa58dccb03ecc1d7b1f6ae888418e8c2fec4b8d".into(),
                ),
            },
            Pick {
                member: "ThirdPartyNotices.txt".into(),
                dest: "ThirdPartyNotices-DirectML.txt".into(),
                sha256: Some(
                    "2c95795c13ff48a58b6ed916f37901c23d964b5d9d601af422f17ad2172e7950".into(),
                ),
            },
        ]),
        // Licencja Microsoftu dopuszcza zbieranie danych diagnostycznych przez bibliotekę — do
        // potwierdzenia przez człowieka (jak biblioteki NVIDIA w wersji CUDA).
        confirmed: false,
        targets: vec![Target::WINDOWS_X64],
        note: note(
            "Biblioteka Microsoftu do obliczeń na karcie graficznej (DirectX 12), potrzebna wersji DirectML biblioteki ONNX Runtime. Pobieranie ma ok. 200 MB, na dysku zostaje ok. 19 MB. Licencja Microsoftu pozwala rozpowszechniać ją z aplikacją, ale dopuszcza zbieranie przez nią danych diagnostycznych (§2a) — dlatego pozycja czeka na potwierdzenie.",
            "Microsoft library for GPU compute (DirectX 12), needed by the DirectML build of ONNX Runtime. The download is about 200 MB, about 19 MB stays on disk. Microsoft's licence allows shipping it with an app but lets it collect diagnostic data (§2a) — so the item awaits confirmation.",
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

    /// Obie pozycje DirectML w jednym katalogu, bez wspólnych plików (usuwanie jednej nie rusza
    /// drugiej), z przypiętymi sumami; DirectML czeka na potwierdzenie licencji.
    #[test]
    fn directml_items_share_a_folder_without_overlap() {
        let (ort, dml) = (onnxruntime_directml(), directml());
        assert_eq!(
            (ort.dir.as_str(), dml.dir.as_str()),
            (ORT_DML_DIR, ORT_DML_DIR)
        );
        assert!(ort.pinned() && dml.pinned() && ort.is_safe() && dml.is_safe());
        assert!(ort.confirmed && !dml.confirmed);
        let (a, b) = (ort.outputs(), dml.outputs());
        assert!(a.contains(&ORT_FILE.to_owned()) && b.contains(&DIRECTML_FILE.to_owned()));
        assert!(a.iter().all(|f| !b.contains(f)), "{a:?} / {b:?}");
        assert!(ort.files[0].url.ends_with(".nupkg") && ort.files[0].url.contains(ORT_DML_VERSION));
        assert!(dml.files[0].url.contains(DIRECTML_VERSION));
        assert_ne!(
            ORT_DML_DIR, ORT_DIR,
            "wersja DirectML nie podmienia biblioteki CPU"
        );
    }
}
