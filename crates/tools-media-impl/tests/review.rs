//! Przegląd poprawności fali 3 (`docs/reviews/2026-10-wave3-review.md`, W3-01): `media_play`
//! nie ufa polom nagłówka WAV. Częstotliwość próbkowania i czas trwania liczone z faktycznie
//! zdekodowanych próbek **przed** resamplingiem — plik z niezaufanego źródła (np. pobrany do
//! kwarantanny) nie wymusi wzmocnienia ×48 000 (1 Hz → 48 kHz), zajęcia gigabajtów pamięci
//! i zablokowania wątku.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use compliance_contract::{DenyLists, PathEnv};
use lib_media::PortFiles;
use platform_fake::FakeFs;
use safety_broker_contract::{Holder, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{Tool, ToolCtx, ToolErrorKind, ToolStatus, Toolset};
use tools_media_contract::MediaToolsConfig;
use tools_media_fake::{FakePlayer, FakeTranscoder};
use tools_media_impl::{MediaTools, MediaToolsDeps};
use undo_journal_contract::{Journal, MemStore, UndoLimits};
use watchdog_contract::ManualClock;

const HOME: &str = "/Users/ala";
const DIR: &str = "/Users/ala/Pobrane";

/// WAV PCM16 mono o częstotliwości `rate`, z polem `byte_rate` podanym wprost (0 albo zawyżone
/// = czas trwania z nagłówka nieznany albo bliski zera) i `samples` próbkami.
fn wav_with_header(rate: u32, byte_rate: u32, samples: u32) -> Vec<u8> {
    let data_len = samples * 2;
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend((0..samples).flat_map(|i| ((i % 64) as i16 * 256).to_le_bytes()));
    out
}

fn tools(files: Vec<(&str, Vec<u8>)>, config: MediaToolsConfig) -> (MediaTools, FakePlayer) {
    let fs = Arc::new(FakeFs::with_files(
        files
            .into_iter()
            .map(|(n, b)| (PathBuf::from(format!("{DIR}/{n}")), b)),
    ));
    let env = PathEnv::windows_profile(HOME);
    let policy = KernelPolicy::baseline(HOME, "/ProgramData/AlfaBroker").unwrap();
    let broker = Arc::new(
        FakeBroker::with(policy, env.clone(), Arc::new(ManualClock::new(1_000_000))).unwrap(),
    );
    broker.script("tools-media.play", ScriptedDecision::Allow);
    let tick = Arc::new(AtomicU64::new(1_000));
    let clock = move || tick.fetch_add(1, Ordering::SeqCst);
    let journal = Arc::new(
        Journal::open(
            fs.clone(),
            Arc::new(MemStore::default()),
            UndoLimits::default(),
            Arc::new(clock),
            1,
        )
        .unwrap(),
    );
    let player = FakePlayer::default();
    let tools = MediaTools::new(MediaToolsDeps {
        fs: fs.clone(),
        files: Arc::new(PortFiles::new(fs)),
        journal,
        transcoder: Arc::new(FakeTranscoder::new()),
        player: Arc::new(player.clone()),
        broker,
        env,
        deny: DenyLists::baseline(),
        config,
        bus: None,
    });
    (tools, player)
}

fn play(t: &MediaTools) -> Arc<dyn Tool> {
    t.tools()
        .into_iter()
        .find(|t| t.manifest().name == "media_play")
        .unwrap()
}

fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta")).with_workdir(DIR);
    c.approval_timeout = Duration::from_millis(300);
    c
}

/// W3-01: 200 próbek „1 Hz” (nagłówek bez `byte_rate` — czas nieznany) dawało przed poprawką klip
/// 9 600 000 próbek 48 kHz (wzmocnienie ×48 000; 64 KiB takiego pliku = ~6 GiB pamięci → przerwanie
/// procesu Alfy przy braku pamięci). Teraz: odmowa przed resamplingiem, odtwarzacz nic nie dostaje.
#[tokio::test]
async fn absurd_sample_rate_is_rejected_before_resampling() {
    let (t, player) = tools(
        vec![("bomba.wav", wav_with_header(1, 0, 200))],
        MediaToolsConfig::default(),
    );
    let out = play(&t).call(json!({"path": "bomba.wav"}), &ctx()).await;
    let played: Vec<usize> = player.clips().iter().map(|c| c.samples.len()).collect();
    assert!(
        played.iter().all(|n| *n <= 200 * 6),
        "wzmocnienie resamplingu ponad ×6 (8 kHz → 48 kHz): {played:?}"
    );
    assert_eq!(
        out.status,
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs
        },
        "{}",
        out.text
    );
    assert!(player.clips().is_empty());
}

/// W3-01: czas trwania z nagłówka (`byte_rate` zawyżone → ~0 ms) nie omija limitu odtwarzania —
/// liczony z próbek przed konwersją częstotliwości (4 kHz → 48 kHz, ×12).
#[tokio::test]
async fn real_duration_is_checked_before_resampling() {
    let config = MediaToolsConfig {
        max_play_ms: 1_000,
        ..MediaToolsConfig::default()
    };
    let (t, player) = tools(
        vec![("dlugi.wav", wav_with_header(4_000, u32::MAX, 8_000))],
        config,
    );
    let out = play(&t).call(json!({"path": "dlugi.wav"}), &ctx()).await;
    assert_eq!(
        out.status,
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs
        },
        "{}",
        out.text
    );
    assert!(player.clips().is_empty());
    // Zwykły plik w dozwolonej częstotliwości nadal gra (bez zmiany zachowania).
    let (t, player) = tools(
        vec![("ok.wav", wav_with_header(8_000, 16_000, 4_000))],
        MediaToolsConfig::default(),
    );
    let ok = play(&t).call(json!({"path": "ok.wav"}), &ctx()).await;
    assert!(ok.is_ok(), "{ok:?}");
    assert_eq!(player.clips()[0].sample_rate, 8_000);
}

fn tool(t: &MediaTools, name: &str) -> Arc<dyn Tool> {
    t.tools()
        .into_iter()
        .find(|t| t.manifest().name == name)
        .unwrap()
}

/// W3-03: ścieżka sieciowa (UNC `\\serwer\udział`, WebDAV `\\host@SSL\…`) od modelu — przed
/// poprawką przechodziła `resolve_path`, a sprawdzenie dowiązań (`canonicalize`) i odczyt nagłówka
/// łączyły się z serwerem napastnika (SMB/WebDAV z automatycznym uwierzytelnieniem NTLM konta
/// właściciela) — część jeszcze **przed** pytaniem Brokera, bez `net.egress`. Teraz: odmowa
/// w postaci ścieżki, zanim cokolwiek dotknie sieci; Broker nie dostaje prośby.
#[tokio::test]
async fn network_paths_from_the_model_are_refused_before_any_access() {
    let (t, player) = tools(
        vec![("ok.wav", wav_with_header(8_000, 16_000, 800))],
        MediaToolsConfig::default(),
    );
    for raw in [
        r"\\napastnik.example\udzial\x.wav",
        r"\\napastnik.example@SSL\DavWWWRoot\x.wav",
        "//napastnik.example/udzial/x.wav",
        r"\/napastnik.example\udzial\x.wav",
    ] {
        for name in ["media_info", "media_play"] {
            let out = tool(&t, name).call(json!({ "path": raw }), &ctx()).await;
            assert_eq!(
                out.status,
                ToolStatus::Failed {
                    error: ToolErrorKind::InvalidArgs
                },
                "{name} {raw}: {}",
                out.text
            );
            assert!(out.text.contains("sieciow"), "{}", out.text);
        }
    }
    assert!(player.clips().is_empty());
    // Ścieżka lokalna działa jak wcześniej.
    let ok = tool(&t, "media_info")
        .call(json!({"path": "ok.wav"}), &ctx())
        .await;
    assert!(ok.is_ok(), "{ok:?}");
}
