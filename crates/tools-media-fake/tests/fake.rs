//! Atrapy przechodzą testy kontraktowe; konwerter i odtwarzacz są deterministyczne.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tools_common_contract::{ToolCtx, ToolOutcome, Toolset};
use tools_media_contract::{
    AudioClip, AudioPlayer, CancellationToken, ConvertError, ConvertJob, TargetFormat, Transcoder,
};
use tools_media_fake::{FakePlayer, FakeTools, FakeTranscoder, sample_output};

#[tokio::test]
async fn contract_and_scripting() {
    let fake = FakeTools::default();
    tools_media_contract::contract_tests::run_all(&fake.tools()).await;
    let first = fake.tools().remove(0);
    let name = first.manifest().name.clone();
    fake.push(&name, ToolOutcome::ok("skrypt", serde_json::json!({})));
    let ctx = ToolCtx::new(safety_broker_contract::Holder::agent("s1", "delta"));
    let out = first
        .call(tools_media_contract::sample_args(&name), &ctx)
        .await;
    assert_eq!(out.text, "skrypt");
    assert_eq!(fake.calls().len(), 1);
}

#[test]
fn transcoder_outputs_valid_headers() {
    let t = FakeTranscoder::new();
    for target in [
        TargetFormat::Wav,
        TargetFormat::Mp3,
        TargetFormat::Png,
        TargetFormat::Gif,
        TargetFormat::Mp4,
    ] {
        let job = ConvertJob {
            input: PathBuf::from("/a.wav"),
            input_format: "wav".into(),
            target,
            start_ms: None,
            duration_ms: None,
            max_side: None,
            max_output_bytes: 1 << 20,
        };
        let bytes = t.convert(&job, Arc::new(AtomicBool::new(false))).unwrap();
        assert_eq!(bytes, sample_output(target));
        assert!(lib_media::probe_bytes(&bytes).is_ok(), "{target:?}");
    }
    assert_eq!(t.jobs().len(), 5);
    let mut job = t.jobs()[0].clone();
    job.max_output_bytes = 4;
    assert_eq!(
        t.convert(&job, Arc::new(AtomicBool::new(false))),
        Err(ConvertError::TooLarge(4))
    );
    assert_eq!(
        t.convert(&job, Arc::new(AtomicBool::new(true))),
        Err(ConvertError::Cancelled)
    );
    t.set_missing(true);
    assert!(matches!(t.available(), Err(ConvertError::NotInstalled(_))));
}

#[tokio::test]
async fn player_records_and_queues() {
    let p = FakePlayer::default();
    let clip = AudioClip {
        samples: vec![0.0; 16_000],
        sample_rate: 16_000,
        channels: 1,
        agent: "alfa".into(),
        label: "a".into(),
    };
    let t = p
        .play(clip.clone(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!((t.id, t.duration_ms, t.queued), (1, 1000, false));
    p.set_busy(true);
    assert!(
        p.play(clip.clone(), CancellationToken::new())
            .await
            .unwrap()
            .queued
    );
    assert_eq!(p.clips().len(), 2);
    p.set_unavailable(true);
    assert!(p.play(clip, CancellationToken::new()).await.is_err());
    assert_eq!(p.stop_all(), 2);
    assert_eq!(p.stops(), 1);
}
