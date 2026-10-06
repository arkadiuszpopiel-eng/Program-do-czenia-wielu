//! `tools-media` na atrapach (FS, konwerter, odtwarzacz, Broker z prawdziwym silnikiem, dziennik
//! cofania): kontrakt, informacje z nagłówków, konwersja jako nowy plik + cofnięcie, nigdy
//! nadpisanie, deny-lista przed Brokerem, brak ffmpeg bez pytania o zgodę, odtwarzanie (WAV,
//! konwersja, resampling, limity), zdarzenia bez treści.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use compliance_contract::{DenyLists, PathEnv};
use core_bus_fake::FakeBus;
use lib_media::{PortFiles, samples};
use platform_contract::FsPort;
use platform_fake::FakeFs;
use safety_broker_contract::{Holder, KernelPolicy};
use safety_broker_fake::{FakeBroker, ScriptedDecision};
use serde_json::json;
use tools_common_contract::{DenialReason, Tool, ToolCtx, ToolErrorKind, ToolStatus, Toolset};
use tools_media_contract::{MediaToolsConfig, TargetFormat};
use tools_media_fake::{FakePlayer, FakeTranscoder, sample_output};
use tools_media_impl::{MediaTools, MediaToolsDeps};
use undo_journal_contract::{Journal, MemStore, StepId, UndoLimits};
use watchdog_contract::ManualClock;

const HOME: &str = "/Users/ala";
const DIR: &str = "/Users/ala/Muzyka";

struct H {
    fs: Arc<FakeFs>,
    broker: Arc<FakeBroker>,
    journal: Arc<Journal>,
    transcoder: FakeTranscoder,
    player: FakePlayer,
    bus: Arc<FakeBus>,
    tools: MediaTools,
}

fn harness(config: MediaToolsConfig) -> H {
    let file = |name: &str, bytes: Vec<u8>| (PathBuf::from(format!("{DIR}/{name}")), bytes);
    let fs = Arc::new(FakeFs::with_files([
        file("glos.wav", samples::wav(16_000, 1, 16, 1500)),
        file("dziwny.wav", samples::wav(32_000, 2, 16, 250)),
        file("piosenka.mp3", samples::mp3(100)),
        file("film.mp4", samples::mp4(1280, 720, 4000, true)),
        file("okladka.png", samples::png(500, 500, 0)),
        file("notatka.txt", b"tekst".to_vec()),
        (
            PathBuf::from("/Users/ala/.ssh/klucz.wav"),
            samples::wav(8000, 1, 16, 10),
        ),
    ]));
    let env = PathEnv::windows_profile(HOME);
    let policy = KernelPolicy::baseline(HOME, "/ProgramData/AlfaBroker").unwrap();
    let broker = Arc::new(
        FakeBroker::with(policy, env.clone(), Arc::new(ManualClock::new(1_000_000))).unwrap(),
    );
    for t in [
        "tools-media.info",
        "tools-media.convert",
        "tools-media.play",
    ] {
        broker.script(t, ScriptedDecision::Allow);
    }
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
    let (transcoder, player, bus) = (
        FakeTranscoder::new(),
        FakePlayer::default(),
        Arc::new(FakeBus::default()),
    );
    let tools = MediaTools::new(MediaToolsDeps {
        fs: fs.clone(),
        files: Arc::new(PortFiles::new(fs.clone())),
        journal: journal.clone(),
        transcoder: Arc::new(transcoder.clone()),
        player: Arc::new(player.clone()),
        broker: broker.clone(),
        env,
        deny: DenyLists::baseline(),
        config,
        bus: Some(bus.clone()),
    });
    H {
        fs,
        broker,
        journal,
        transcoder,
        player,
        bus,
        tools,
    }
}

fn h() -> H {
    harness(MediaToolsConfig::default())
}

fn ctx() -> ToolCtx {
    let mut c = ToolCtx::new(Holder::agent("s1", "delta")).with_workdir(DIR);
    c.approval_timeout = Duration::from_millis(300);
    c
}

impl H {
    fn tool(&self, name: &str) -> Arc<dyn Tool> {
        self.tools
            .tools()
            .into_iter()
            .find(|t| t.manifest().name == name)
            .unwrap()
    }
}

#[tokio::test]
async fn contract_suite() {
    tools_media_contract::contract_tests::run_all(&h().tools.tools()).await;
}

#[tokio::test]
async fn info_reads_headers_through_the_broker() {
    let h = h();
    let info = h.tool("media_info");
    let mp4 = info.call(json!({"path": "film.mp4"}), &ctx()).await;
    assert!(mp4.is_ok(), "{mp4:?}");
    assert_eq!(mp4.data["info"]["width"], 1280);
    assert_eq!(mp4.data["info"]["duration_ms"], 4000);
    assert!(
        mp4.text.contains("h264") && mp4.text.contains("0:04.000"),
        "{}",
        mp4.text
    );
    assert_eq!(mp4.untrusted, None);
    let wav = info.call(json!({"path": "glos.wav"}), &ctx()).await;
    assert_eq!(wav.data["info"]["sample_rate"], 16_000);
    let denied = info
        .call(json!({"path": "/Users/ala/.ssh/klucz.wav"}), &ctx())
        .await;
    assert_eq!(
        denied.status,
        ToolStatus::Denied {
            reason: DenialReason::DenyList
        }
    );
    let text = info.call(json!({"path": "notatka.txt"}), &ctx()).await;
    assert_eq!(
        text.status,
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs
        }
    );
    let missing = info.call(json!({"path": "brak.wav"}), &ctx()).await;
    assert_eq!(
        missing.status,
        ToolStatus::Failed {
            error: ToolErrorKind::NotFound
        }
    );
    h.broker
        .script("tools-media.info", ScriptedDecision::NeedsApproval);
    let refused = info.call(json!({"path": "glos.wav"}), &ctx()).await;
    assert!(matches!(refused.status, ToolStatus::Denied { .. }));
    let events: String = h
        .bus
        .recorded()
        .iter()
        .map(|e| e.payload.to_string())
        .collect();
    assert!(
        !events.contains("Muzyka"),
        "bez ścieżek w zdarzeniach: {events}"
    );
}

#[tokio::test]
async fn convert_writes_a_new_file_that_undo_removes() {
    let h = h();
    let convert = h.tool("media_convert");
    let out = convert
        .call(
            json!({"path": "film.mp4", "format": "mp3", "start_s": 1, "duration_s": 2.5}),
            &ctx(),
        )
        .await;
    assert!(out.is_ok(), "{out:?}");
    let new = format!("{DIR}/film (Alfa).mp3");
    assert_eq!(out.data["output"], new);
    assert_eq!(
        h.fs.read(Path::new(&new)).unwrap(),
        sample_output(TargetFormat::Mp3)
    );
    assert_eq!(
        h.fs.read(Path::new(&format!("{DIR}/film.mp4"))).unwrap(),
        samples::mp4(1280, 720, 4000, true)
    );
    let job = &h.transcoder.jobs()[0];
    assert_eq!(
        (
            job.input_format.as_str(),
            job.target,
            job.start_ms,
            job.duration_ms
        ),
        ("mp4", TargetFormat::Mp3, Some(1000), Some(2500))
    );
    let undo = out.undo.clone().unwrap();
    let second = convert
        .call(json!({"path": "film.mp4", "format": "mp3"}), &ctx())
        .await;
    assert_eq!(second.data["output"], format!("{DIR}/film (Alfa 2).mp3"));
    h.journal.undo(StepId(undo.id)).unwrap();
    assert!(!h.fs.exists(Path::new(&new)), "cofnięcie usuwa nowy plik");
    assert!(
        h.bus
            .recorded()
            .iter()
            .any(|e| e.kind.as_str() == "tool.media.convert")
    );
}

#[tokio::test]
async fn convert_never_overwrites_and_checks_paths_first() {
    let h = h();
    let convert = h.tool("media_convert");
    let exists = convert
        .call(
            json!({"path": "film.mp4", "format": "png", "output": "okladka.png"}),
            &ctx(),
        )
        .await;
    assert_eq!(
        exists.status,
        ToolStatus::Failed {
            error: ToolErrorKind::AlreadyExists
        }
    );
    let same = convert
        .call(
            json!({"path": "glos.wav", "format": "wav", "output": "glos.wav"}),
            &ctx(),
        )
        .await;
    assert_eq!(
        same.status,
        ToolStatus::Denied {
            reason: DenialReason::Policy
        }
    );
    let secret = convert
        .call(
            json!({"path": "glos.wav", "format": "mp3", "output": "/Users/ala/.ssh/x.mp3"}),
            &ctx(),
        )
        .await;
    assert_eq!(
        secret.status,
        ToolStatus::Denied {
            reason: DenialReason::DenyList
        }
    );
    let wrong = convert
        .call(json!({"path": "glos.wav", "format": "mp4"}), &ctx())
        .await;
    assert_eq!(
        wrong.status,
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs
        }
    );
    let text = convert
        .call(json!({"path": "notatka.txt", "format": "mp3"}), &ctx())
        .await;
    assert_eq!(
        text.status,
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs
        }
    );
    assert!(h.transcoder.jobs().is_empty());
    h.transcoder.set_missing(true);
    h.broker
        .script("tools-media.convert", ScriptedDecision::NeedsApproval);
    let missing = convert
        .call(json!({"path": "film.mp4", "format": "mp3"}), &ctx())
        .await;
    assert_eq!(
        missing.status,
        ToolStatus::Failed {
            error: ToolErrorKind::Unsupported
        },
        "bez pytania o zgodę"
    );
    assert!(missing.text.contains("ffmpeg"));
    h.transcoder.set_missing(false);
    h.transcoder
        .push(Err(tools_media_contract::ConvertError::Timeout));
    h.broker
        .script("tools-media.convert", ScriptedDecision::Allow);
    let timeout = convert
        .call(json!({"path": "film.mp4", "format": "mp3"}), &ctx())
        .await;
    assert_eq!(
        timeout.status,
        ToolStatus::Failed {
            error: ToolErrorKind::Timeout
        }
    );
    assert!(
        !h.fs.exists(Path::new(&format!("{DIR}/film (Alfa).mp3"))),
        "błąd = brak pliku"
    );
}

#[tokio::test]
async fn play_queues_clips_for_the_speaker() {
    let h = h();
    let play = h.tool("media_play");
    let wav = play.call(json!({"path": "glos.wav"}), &ctx()).await;
    assert!(wav.is_ok(), "{wav:?}");
    assert_eq!(wav.data["converted"], false);
    let clip = &h.player.clips()[0];
    assert_eq!(
        (clip.sample_rate, clip.channels, clip.agent.as_str()),
        (16_000, 1, "delta")
    );
    assert_eq!(clip.duration_ms(), 1500);
    let odd = play.call(json!({"path": "dziwny.wav"}), &ctx()).await;
    assert!(odd.is_ok(), "{odd:?}");
    assert_eq!(
        h.player.clips()[1].sample_rate,
        48_000,
        "32 kHz → 48 kHz, mono"
    );
    h.player.set_busy(true);
    let mp3 = play.call(json!({"path": "piosenka.mp3"}), &ctx()).await;
    assert!(
        mp3.is_ok() && mp3.data["converted"] == true && mp3.data["queued"] == true,
        "{mp3:?}"
    );
    assert_eq!(
        h.transcoder.jobs().last().unwrap().target,
        TargetFormat::Wav
    );
    let image = play.call(json!({"path": "okladka.png"}), &ctx()).await;
    assert_eq!(
        image.status,
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs
        }
    );
    h.transcoder.set_missing(true);
    let no_ffmpeg = play.call(json!({"path": "piosenka.mp3"}), &ctx()).await;
    assert_eq!(
        no_ffmpeg.status,
        ToolStatus::Failed {
            error: ToolErrorKind::Unsupported
        }
    );
    h.player.set_unavailable(true);
    let silent = play.call(json!({"path": "glos.wav"}), &ctx()).await;
    assert_eq!(
        silent.status,
        ToolStatus::Failed {
            error: ToolErrorKind::Unsupported
        }
    );
    assert_eq!(h.tools.stop_all(), 3);
}

#[tokio::test]
async fn play_limits_length() {
    let h = harness(MediaToolsConfig {
        max_play_ms: 1000,
        ..MediaToolsConfig::default()
    });
    let out = h
        .tool("media_play")
        .call(json!({"path": "glos.wav"}), &ctx())
        .await;
    assert_eq!(
        out.status,
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs
        }
    );
    assert!(out.text.contains("media_convert"));
    assert!(h.player.clips().is_empty());
}
