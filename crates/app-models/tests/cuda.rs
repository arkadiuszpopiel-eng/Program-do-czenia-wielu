//! `llama-server` CUDA (fala 5): pozycja katalogu „do potwierdzenia” z dwoma archiwami wydania
//! (serwer + `cudart`), instalacja obu do jednego drzewa, odrzucenie pliku powtórzonego
//! w dwóch archiwach (bez skutków ubocznych).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_api::dto::{ModelItemKind as K, ModelItemState as S};
use app_models::catalog::{Install, Root};
use app_models::unpack::{extract_trees, staging_dir};
use common::http::Server;
use common::{harness, options, spec};
use updater_contract::PackageLimits;

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut out = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut out);
    for (name, bytes) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
    out.into_inner()
}

#[test]
fn catalog_has_unconfirmed_cuda_build_with_cudart() {
    let items = app_models::builtin();
    let cuda = items
        .iter()
        .find(|i| i.id == "sidecar-llama-cuda")
        .expect("pozycja CUDA w katalogu");
    assert_eq!(cuda.kind, K::Sidecar);
    assert_eq!(
        (cuda.root, cuda.dir.as_str()),
        (Root::Sidecars, "llama-cuda")
    );
    assert!(!cuda.confirmed, "do potwierdzenia przez człowieka");
    assert!(!cuda.pinned(), "bez zgadywania hashy");
    assert_eq!(cuda.files.len(), 2);
    assert!(cuda.files.iter().all(|f| {
        f.url
            .starts_with("https://github.com/ggml-org/llama.cpp/releases/download/")
    }));
    assert!(cuda.files[1].url.contains("/cudart-llama-bin-win-cuda-"));
    let Install::Tree { require, .. } = &cuda.install else {
        panic!("instalacja drzewem: {:?}", cuda.install);
    };
    assert!(
        require.iter().any(|r| r == "cudart64_12.dll"),
        "{require:?}"
    );
    for backend in ["vulkan", "cpu"] {
        let id = format!("sidecar-llama-{backend}");
        assert!(items.iter().any(|i| i.id == id), "{id}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_and_cudart_archives_install_into_one_tree() {
    let server = Server::start().await;
    let srv = zip_of(&[
        ("llama-server", b"exe" as &[u8]),
        ("ggml-cuda.dll", b"cuda"),
    ]);
    let rt = zip_of(&[
        ("cudart64_12.dll", b"rt" as &[u8]),
        ("cublas64_12.dll", b"blas"),
    ]);
    let item = spec(
        "llama-cuda",
        K::Sidecar,
        &server,
        &[
            ("llama-cuda.zip", "/llama-cuda.zip", &srv, true),
            ("cudart.zip", "/cudart.zip", &rt, true),
        ],
        Install::Tree {
            strip: String::new(),
            require: vec!["llama-server".into(), "cudart64_12.dll".into()],
        },
    );
    let h = harness(vec![item], None, options(2));
    h.app.download("llama-cuda").await.unwrap();
    h.wait("llama-cuda", S::Installed).await;
    let dir = h.paths.models().join("t/llama-cuda");
    for (file, bytes) in [
        ("llama-server", b"exe" as &[u8]),
        ("ggml-cuda.dll", b"cuda"),
        ("cudart64_12.dll", b"rt"),
        ("cublas64_12.dll", b"blas"),
    ] {
        assert_eq!(std::fs::read(dir.join(file)).unwrap(), bytes, "{file}");
    }
    assert!(!dir.join("llama-cuda.zip").exists() && !dir.join("cudart.zip").exists());
    std::fs::remove_file(dir.join("cudart64_12.dll")).unwrap();
    assert_eq!(h.app.verify("llama-cuda").await.unwrap().state, S::Corrupt);
}

#[test]
fn file_repeated_in_two_archives_rejects_everything() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("sidecars").join("llama-cuda");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("llama-server"), b"poprzednia").unwrap();
    let a = dir.path().join("a.zip");
    let b = dir.path().join("b.zip");
    std::fs::write(
        &a,
        zip_of(&[("llama-server", b"nowy" as &[u8]), ("x.dll", b"1")]),
    )
    .unwrap();
    std::fs::write(&b, zip_of(&[("X.DLL", b"podmiana" as &[u8])])).unwrap();
    let require = ["llama-server".to_owned()];
    let err = extract_trees(
        &[a.clone(), b],
        &target,
        "",
        &require,
        PackageLimits::default(),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("więcej niż jednym archiwum"),
        "{err}"
    );
    assert_eq!(
        std::fs::read(target.join("llama-server")).unwrap(),
        b"poprzednia"
    );
    assert!(!staging_dir(&target).exists());
    assert!(extract_trees(&[], &target, "", &require, PackageLimits::default()).is_err());
    let ok = extract_trees(&[a], &target, "", &require, PackageLimits::default()).unwrap();
    assert_eq!(ok.len(), 2);
}
