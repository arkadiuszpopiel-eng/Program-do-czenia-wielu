//! Jawna platforma katalogu i nazwy z archiwów są niezależne od hosta testu.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use app_models::catalog::{Install, Target, builtin, builtin_for};

#[test]
fn host_catalog_keeps_its_filter_and_windows_can_be_inspected_elsewhere() {
    assert_eq!(builtin(), builtin_for(Target::host()));
    let windows = builtin_for(Some(Target::WINDOWS_X64));
    for id in [
        "sidecar-llama-cuda",
        "sidecar-llama-cpu",
        "directml",
        "onnxruntime-cpu",
    ] {
        assert!(windows.iter().any(|i| i.id == id), "{id}");
    }
    for target in [
        Target::parse("linux", "x86_64"),
        Target::parse("macos", "aarch64"),
        None,
    ] {
        let catalog = builtin_for(target);
        assert!(catalog.iter().all(|i| i.targets.is_empty()));
        assert!(!catalog.iter().any(|i| i.id == "sidecar-llama-cuda"));
        assert!(
            catalog
                .iter()
                .any(|i| i.id == "bielik-1.5b-v3.0-instruct-q8_0")
        );
    }
}

#[test]
fn windows_archive_metadata_requires_exe_even_on_linux() {
    let windows = builtin_for(Some(Target::WINDOWS_X64));
    let llama = windows
        .iter()
        .find(|i| i.id == "sidecar-llama-cpu")
        .unwrap();
    let Install::Tree { require, .. } = &llama.install else {
        panic!("instalacja drzewa")
    };
    assert!(require.iter().any(|n| n == "llama-server.exe"));
    assert_eq!(Target::WINDOWS_X64.executable("piper"), "piper.exe");
    assert_eq!(
        Target::parse("linux", "x86_64")
            .unwrap()
            .executable("piper"),
        "piper"
    );
}

#[test]
fn portable_manual_installations_keep_the_selected_platforms_names() {
    for (target, expected) in [
        (Some(Target::WINDOWS_X64), "ffmpeg.exe"),
        (Target::parse("linux", "x86_64"), "ffmpeg"),
        (None, "ffmpeg"),
    ] {
        let items = builtin_for(target);
        let ffmpeg = items.iter().find(|i| i.id == "sidecar-ffmpeg").unwrap();
        let Install::Manual(files) = &ffmpeg.install else {
            panic!("instalacja ręczna")
        };
        assert_eq!(files, &[expected.to_owned()]);
    }
}
