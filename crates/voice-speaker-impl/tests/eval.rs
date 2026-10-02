//! Runner EER na próbkach syntetycznych CI (`evals/F5/voice/samples/speaker.ndjson`) z modelem
//! atrapy (`PitchEmbedder`): walidacja manifestu, EER/F5-07, profil tymczasowy, CLI, schemat.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use voice_speaker_contract::SpeakerCfg;
use voice_speaker_fake::PitchEmbedder;
use voice_speaker_impl::eval::{manifest_schema, parse_manifest, run, validate_manifest};

fn evals() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals/F5/voice")
}

#[test]
fn synthetic_samples_eer() {
    let text = std::fs::read_to_string(evals().join("samples/speaker.ndjson")).unwrap();
    let items = parse_manifest(&text).unwrap();
    assert!(
        validate_manifest(&items).is_empty(),
        "{:?}",
        validate_manifest(&items)
    );
    let (r, trials) = run(
        &items,
        Path::new("."),
        PitchEmbedder::new(),
        SpeakerCfg::default(),
        "test",
    )
    .unwrap();
    eprintln!(
        "F5-07 (syntetyczne): EER {:.4}, właściciel {}, obcy {}, przy progach {:?}",
        r.eer, r.genuine, r.impostor, r.at_config
    );
    assert_eq!((r.genuine, r.impostor), (10, 24));
    assert_eq!(trials.len(), 34);
    assert!(r.f5_07_ok && r.f5_08_ok && !r.sufficient_impostors);
}

#[test]
fn manifest_validation_and_cli() {
    let bad = parse_manifest(
        r#"{"id":"X","audio":"../a.wav","speaker":"cv:1","role":"enroll","split":"prod","source":"common_voice"}"#,
    )
    .unwrap();
    let p = validate_manifest(&bad).join("\n");
    for needle in [
        "identyfikator",
        "audio",
        "tylko głosu właściciela",
        "split",
        "rejestracja: 1",
    ] {
        assert!(p.contains(needle), "{needle} ∉ {p}");
    }
    let exe = env!("CARGO_BIN_EXE_alfa-speaker-eval");
    let manifest = evals().join("samples/speaker.ndjson");
    let out = std::process::Command::new(exe)
        .args(["check", manifest.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("pozycji: 37"));
    let schema = serde_json::to_string_pretty(&manifest_schema()).unwrap() + "\n";
    let path = evals().join("speaker-manifest.schema.json");
    if std::env::var_os("ALFA_UPDATE_SCHEMAS").is_some() {
        std::fs::write(&path, &schema).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(&path).unwrap_or_default(),
        schema,
        "schemat nieaktualny — uruchom z ALFA_UPDATE_SCHEMAS=1"
    );
}
