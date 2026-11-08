//! Implementacja: kontrakt na grafie ONNX zbudowanym w teście (manifest z SHA-256), profil
//! zaszyfrowany (brak jawnych danych w pliku, manipulacja wykryta, usunięcie = crypto-shredding),
//! bramka właściciela dla słów wywoławczych, moduł. Prawdziwy model: `real_model` (`#[ignore]`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::sync::Arc;

use common::onnx::{int_attr, int64s, ints_attr, model, node, value};
use sessions_contract::KeyVault;
use sessions_fake::MemoryKeyVault;
use voice_speaker_contract::{
    Embedding, Profile, ProfileStore, SpeakerCfg, SpeakerError, SpeakerVerifier, contract_tests,
};
use voice_speaker_fake::{owner_voice, stranger_voice};
use voice_speaker_impl::{
    EncryptedFileStore, KEY_NAME, MAGIC, MODULE_TOML, OnnxSpeakerModel, SpeakerOwnerCheck,
    VoiceSpeakerModule, open_speaker, sha256_hex,
};
use voice_wake_contract::OwnerCheck;

const BINS: i64 = 24;

/// „Embedding”: średni log-mel po czasie, najniższe 24 pasma (harmoniczne F0), bez średniej,
/// potem `exp` — wektor skupiony na szczytach harmonicznych (głosy o innym F0 → inne pasma).
fn speaker_onnx() -> Vec<u8> {
    model(
        value("feats", &[1, 200, 80]),
        value("emb", &[1, BINS]),
        vec![
            node(
                "ReduceMean",
                &["feats"],
                "m",
                vec![ints_attr("axes", &[1]), int_attr("keepdims", 0)],
            ),
            node("Slice", &["m", "st", "en", "ax"], "low", vec![]),
            node(
                "ReduceMean",
                &["low"],
                "avg",
                vec![ints_attr("axes", &[1]), int_attr("keepdims", 1)],
            ),
            node("Sub", &["low", "avg"], "centered", vec![]),
            node("Exp", &["centered"], "emb", vec![]),
        ],
        vec![
            int64s("st", vec![0]),
            int64s("en", vec![BINS]),
            int64s("ax", vec![1]),
        ],
    )
}

fn write_model(dir: &Path) -> std::path::PathBuf {
    let bytes = speaker_onnx();
    std::fs::write(dir.join("spk.onnx"), &bytes).unwrap();
    let m = serde_json::json!({
        "format": "alfa-speaker-v1", "name": "test-lowband-v1", "license": "test",
        "path": "spk.onnx", "sha256": sha256_hex(&bytes),
        "n_mels": 80, "frames": 200, "hop_frames": 100, "cmn": false
    });
    let p = dir.join("spk.speaker.json");
    std::fs::write(&p, m.to_string()).unwrap();
    p
}

/// Progi dopasowane do prostego modelu testowego (prawdziwe — z runnera EER na korpusie).
fn test_cfg() -> SpeakerCfg {
    SpeakerCfg {
        // Deterministycznie: właściciel 0,78–0,99, obcy ≤ 0,74 (ziarna 1–19).
        threshold_standard: 0.76,
        threshold_strict: 0.9,
        ..SpeakerCfg::default()
    }
}

#[test]
fn contract_suite_on_onnx_model_with_encrypted_store() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = write_model(dir.path());
    let profile = dir.path().join("voice/speaker.bin");
    let vault: Arc<dyn KeyVault> = Arc::new(MemoryKeyVault::new());
    contract_tests::run_all(
        || {
            let _ = std::fs::remove_file(&profile);
            open_speaker(&manifest, &profile, Arc::clone(&vault), test_cfg()).unwrap()
        },
        &owner_voice,
        &stranger_voice,
    );
}

#[test]
fn encrypted_store_hides_data_detects_tampering_and_shreds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("speaker.bin");
    let vault = Arc::new(MemoryKeyVault::new());
    let store = EncryptedFileStore::new(&path, vault.clone());
    assert_eq!(store.load().unwrap(), None);
    let p = Profile {
        model: "ecapa-tajny-model".into(),
        utterances: 4,
        embedding: Embedding::new(vec![0.25; 192]).unwrap(),
    };
    store.save(&p).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..8], MAGIC);
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains("ecapa") && !text.contains("embedding") && !text.contains("0.07"));
    let back = store.load().unwrap().unwrap();
    assert_eq!(
        (back.model.as_str(), back.utterances),
        ("ecapa-tajny-model", 4)
    );
    assert!((voice_speaker_contract::cosine(&back.embedding, &p.embedding) - 1.0).abs() < 1e-5);
    assert_eq!(vault.names(), vec![KEY_NAME.to_owned()]);
    let mut tampered = bytes.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    std::fs::write(&path, &tampered).unwrap();
    assert!(matches!(store.load(), Err(SpeakerError::Crypto(_))));
    std::fs::write(&path, b"ALFASPK0xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx").unwrap();
    assert!(matches!(store.load(), Err(SpeakerError::Crypto(_))));
    std::fs::write(&path, &bytes).unwrap();
    assert!(store.delete().unwrap());
    assert!(
        !path.exists() && vault.names().is_empty(),
        "plik i klucz usunięte"
    );
    // Kopia pliku z kopii zapasowej bez klucza jest bezużyteczna.
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(store.load().unwrap(), None);
    assert!(!format!("{store:?}").contains("key"));
}

#[test]
fn manifest_hash_is_enforced_and_owner_gate_is_fail_closed() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = write_model(dir.path());
    std::fs::write(dir.path().join("spk.onnx"), b"podmieniony").unwrap();
    let e = OnnxSpeakerModel::load(&manifest).unwrap_err();
    assert!(e.to_string().contains("SHA-256"), "{e}");
    let manifest = write_model(dir.path());
    let profile = dir.path().join("p.bin");
    let verifier = Arc::new(
        open_speaker(
            &manifest,
            &profile,
            Arc::new(MemoryKeyVault::new()),
            test_cfg(),
        )
        .unwrap(),
    );
    let mut gate = SpeakerOwnerCheck(verifier.clone());
    assert_eq!(gate.is_owner(&owner_voice(1)), None, "bez profilu — odmowa");
    verifier.begin_enrollment().unwrap();
    for s in 1..=3 {
        verifier.add_enrollment(&owner_voice(s)).unwrap();
    }
    verifier.finish_enrollment().unwrap();
    assert_eq!(gate.is_owner(&owner_voice(7)), Some(true));
    assert_eq!(gate.is_owner(&stranger_voice(7)), Some(false));
}

#[tokio::test]
async fn module_manifest_and_events() {
    let m = VoiceSpeakerModule::new().unwrap();
    assert!(MODULE_TOML.contains("voice-speaker"));
    assert_eq!(m.publish(&[]).await, 0);
}

/// Prawdziwy model (np. WeSpeaker/3D-Speaker z sherpa-onnx): `ALFA_SPEAKER_MODEL=…/x.speaker.json`,
/// `ALFA_SPEAKER_WAVS=a.wav,b.wav,c.wav,d.wav` (16 kHz mono; 3 pierwsze — rejestracja).
#[test]
#[ignore = "wymaga modelu mówcy spoza repo (ALFA_SPEAKER_MODEL)"]
fn real_model() {
    let (Ok(model), Ok(wavs)) = (
        std::env::var("ALFA_SPEAKER_MODEL"),
        std::env::var("ALFA_SPEAKER_WAVS"),
    ) else {
        eprintln!("ALFA_SPEAKER_MODEL/ALFA_SPEAKER_WAVS nie ustawione — pomijam");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let v = open_speaker(
        Path::new(&model),
        &dir.path().join("p.bin"),
        Arc::new(MemoryKeyVault::new()),
        SpeakerCfg::default(),
    )
    .unwrap();
    let load = |p: &str| {
        let (pcm, _) = voice_audio_contract::wav::decode_wav(&std::fs::read(p).unwrap()).unwrap();
        pcm
    };
    let files: Vec<&str> = wavs.split(',').collect();
    v.begin_enrollment().unwrap();
    for f in &files[..3] {
        v.add_enrollment(&load(f)).unwrap();
    }
    v.finish_enrollment().unwrap();
    for f in &files[3..] {
        let t0 = std::time::Instant::now();
        eprintln!(
            "{f}: {:?} w {:?}",
            v.verify(&load(f)).unwrap(),
            t0.elapsed()
        );
    }
}
