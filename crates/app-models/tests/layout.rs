//! Układ archiwów z prawdziwych wydań (fala 6, próba generalna przed testem na laptopie):
//! pozycje sidecarów z **wbudowanego katalogu** (`builtin()`: te same nazwy plików, `strip`,
//! `require` i katalogi docelowe) instalowane przez `ModelsApp` (pobranie → zgoda TOFU →
//! rozpakowanie) z lokalnego serwera z archiwami zbudowanymi w teście o układzie jak w wydaniach.
//!
//! Źródło wiedzy o układzie — skrypty pakujące wydań (stan wiedzy z 2025 r.; do potwierdzenia jobem
//! „Silniki na żywo (CPU)” w `.github/workflows/rehearsal.yml`, który wypisuje faktyczny układ):
//! - llama.cpp `b6710`: `7z a llama-…-bin-win-<backend>-x64.zip .\build\bin\Release\*` — pliki
//!   w korzeniu (`llama-server.exe`, `llama.dll`, `ggml*.dll`, warianty `ggml-cpu-*.dll`);
//!   `cudart-llama-bin-win-cuda-12.4-x64.zip` — same biblioteki CUDA w korzeniu;
//! - whisper.cpp `v1.8.1`: `Compress-Archive -Path build/bin/Release` — katalog `Release/`;
//! - piper `2023.11.14-2`: katalog `piper/` z `piper.exe`, bibliotekami i `espeak-ng-data/`.
//!
//! Warianty odporności: separator `\` (Compress-Archive w Windows PowerShell 5.1) i archiwum bez
//! prefiksu z katalogu (pliki w korzeniu) — instalacja działa, a brak wymaganego pliku daje błąd
//! z listą wpisów archiwum.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::Write;

use app_api::dto::{ModelItemState as S, TrustedHashes};
use app_models::catalog::{ItemSpec, exe};
use app_models::unpack::extract_tree;
use common::http::Server;
use common::{harness, options};
use updater_contract::PackageLimits;

fn zip_of(entries: &[(String, &[u8])]) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut out);
    for (name, bytes) in entries {
        zip.start_file(name.as_str(), zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
    out.into_inner()
}

fn files(names: &[&str]) -> Vec<(String, &'static [u8])> {
    names
        .iter()
        .map(|n| ((*n).to_owned(), b"bin" as &[u8]))
        .collect()
}

/// Pozycja z wbudowanego katalogu z adresami podmienionymi na lokalny serwer.
fn local_copy(id: &str, server: &Server, archives: &[Vec<u8>]) -> ItemSpec {
    let mut spec = app_models::builtin()
        .into_iter()
        .find(|i| i.id == id)
        .unwrap_or_else(|| panic!("{id} w katalogu"));
    assert_eq!(spec.files.len(), archives.len(), "{id}: liczba archiwów");
    for (f, bytes) in spec.files.iter_mut().zip(archives) {
        let path = format!("/{id}/{}", f.name);
        server.put(&path, bytes.clone());
        f.url = format!("{}{path}", server.base);
        f.size = bytes.len() as u64;
    }
    spec
}

/// Pozycja katalogu, jej archiwa i pliki oczekiwane po instalacji.
type Case = (&'static str, Vec<Vec<u8>>, Vec<String>);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_sidecars_install_from_release_like_archives() {
    let server = Server::start().await;
    let llama = |backend: &str| {
        let mut names = vec![
            exe("llama-server"),
            exe("llama-cli"),
            "llama.dll".into(),
            "ggml.dll".into(),
            "ggml-base.dll".into(),
            "ggml-cpu-haswell.dll".into(),
            "ggml-cpu-alderlake.dll".into(),
            "mtmd.dll".into(),
        ];
        names.push(format!("ggml-{backend}.dll"));
        zip_of(&files(
            &names.iter().map(String::as_str).collect::<Vec<_>>(),
        ))
    };
    let cudart = zip_of(&files(&[
        "cudart64_12.dll",
        "cublas64_12.dll",
        "cublasLt64_12.dll",
    ]));
    let whisper = zip_of(&files(&[
        &format!("Release/{}", exe("whisper-server")),
        &format!("Release/{}", exe("whisper-cli")),
        "Release/whisper.dll",
        "Release/ggml.dll",
        "Release/SDL2.dll",
    ]));
    let piper = zip_of(&files(&[
        &format!("piper/{}", exe("piper")),
        "piper/onnxruntime.dll",
        "piper/espeak-ng.dll",
        "piper/piper_phonemize.dll",
        "piper/espeak-ng-data/phontab",
        "piper/espeak-ng-data/pl_dict",
        "piper/espeak-ng-data/voices/!v/f1",
    ]));
    let cases: Vec<Case> = vec![
        (
            "sidecar-llama-cpu",
            vec![llama("cpu")],
            vec![exe("llama-server")],
        ),
        (
            "sidecar-llama-vulkan",
            vec![llama("vulkan")],
            vec![exe("llama-server")],
        ),
        (
            "sidecar-llama-cuda",
            vec![llama("cuda"), cudart],
            vec![
                exe("llama-server"),
                "cudart64_12.dll".into(),
                "ggml-cuda.dll".into(),
            ],
        ),
        (
            "sidecar-whisper-cpu",
            vec![whisper],
            vec![exe("whisper-server"), "SDL2.dll".into()],
        ),
        (
            "sidecar-piper",
            vec![piper],
            vec![
                exe("piper"),
                "espeak-ng-data/pl_dict".into(),
                "espeak-ng-data/voices/!v/f1".into(),
            ],
        ),
    ];
    let specs: Vec<ItemSpec> = cases
        .iter()
        .map(|(id, archives, _)| local_copy(id, &server, archives))
        .collect();
    let h = harness(specs.clone(), None, options(2));
    for ((id, _, expected), spec) in cases.iter().zip(&specs) {
        h.app.download(id).await.unwrap();
        let pending = h.wait(id, S::NeedsTrust).await;
        let shown: TrustedHashes = pending
            .files
            .iter()
            .map(|f| (f.name.clone(), f.sha256.clone().unwrap()))
            .collect();
        h.app.trust_hash(id, shown).await.unwrap();
        let item = h.wait(id, S::Installed).await;
        let target = spec.target(&h.paths);
        assert_eq!(item.target, target.display().to_string());
        for file in expected {
            assert!(target.join(file).is_file(), "{id}: brak {file}");
        }
        assert!(
            !target.join("Release").is_dir() && !target.join("piper").is_dir(),
            "{id}: prefiks archiwum usunięty"
        );
    }
    // Ścieżki, pod którymi szuka ich aplikacja (`AppPaths::sidecar`, app-modules / app-voice).
    for (dir, file) in [
        ("llama-cpu", "llama-server"),
        ("llama-vulkan", "llama-server"),
        ("llama-cuda", "llama-server"),
        ("whisper", "whisper-server"),
        ("piper", "piper"),
    ] {
        assert!(h.paths.sidecar(dir, file).is_file(), "{dir}/{file}");
    }
}

#[test]
fn windows_separators_and_missing_prefix_are_tolerated() {
    let dir = tempfile::tempdir().unwrap();
    // Compress-Archive (PowerShell 5.1): `\` w nazwach, także wpis katalogu zakończony `\`.
    let ps51 = dir.path().join("piper.zip");
    std::fs::write(
        &ps51,
        zip_of(&[
            ("piper\\piper".into(), b"exe" as &[u8]),
            ("piper\\espeak-ng-data\\".into(), b""),
            ("piper\\espeak-ng-data\\pl_dict".into(), b"dict"),
        ]),
    )
    .unwrap();
    let target = dir.path().join("sidecars").join("piper");
    let hashes = extract_tree(
        &ps51,
        &target,
        "piper/",
        &["piper".into()],
        PackageLimits::default(),
    )
    .unwrap();
    assert!(target.join("espeak-ng-data").join("pl_dict").is_file());
    assert!(hashes.contains_key("espeak-ng-data/pl_dict"), "{hashes:?}");
    // Wydanie bez katalogu `Release/` (pliki w korzeniu) — instalacja i tak działa.
    let flat = dir.path().join("whisper.zip");
    std::fs::write(&flat, zip_of(&files(&["whisper-server", "ggml.dll"]))).unwrap();
    let target = dir.path().join("sidecars").join("whisper");
    extract_tree(
        &flat,
        &target,
        "Release/",
        &["whisper-server".into()],
        PackageLimits::default(),
    )
    .unwrap();
    assert!(target.join("whisper-server").is_file());
    // Inny układ bez wymaganego pliku w korzeniu → błąd z listą wpisów (diagnoza z UI).
    let other = dir.path().join("other.zip");
    std::fs::write(&other, zip_of(&files(&["bin/whisper-server", "README.md"]))).unwrap();
    let err = extract_tree(
        &other,
        &target,
        "Release/",
        &["whisper-server".into()],
        PackageLimits::default(),
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("do potwierdzenia") && err.contains("bin/") && err.contains("README.md"),
        "{err}"
    );
    assert!(
        target.join("whisper-server").is_file(),
        "poprzednia instalacja nietknięta"
    );
    // `..\` po normalizacji to nadal `../` — odrzucone.
    let evil = dir.path().join("evil.zip");
    std::fs::write(
        &evil,
        zip_of(&[("piper\\..\\..\\evil".into(), b"x" as &[u8])]),
    )
    .unwrap();
    assert!(extract_tree(&evil, &target, "piper/", &[], PackageLimits::default()).is_err());
    assert!(!dir.path().join("evil").exists());
}
