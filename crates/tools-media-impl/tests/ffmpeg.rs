//! `FfmpegTranscoder` na atrapie `ExecPort` i prawdziwym katalogu roboczym: argumenty z listy
//! zamkniętej (wymuszony demuxer, tylko `file:`, bez metadanych, `-n`), środowisko bez sekretów,
//! Job Object z limitem pamięci, wynik odczytany i plik tymczasowy usunięty, błędy, limit czasu,
//! anulowanie, za duży wynik.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use platform_contract::ExecSpec;
use platform_fake::{FakeExec, FakeRun};
use tools_media_contract::{ConvertError, ConvertJob, TargetFormat, Transcoder};
use tools_media_impl::{FfmpegConfig, FfmpegTranscoder, ffmpeg_args};

fn job(input_format: &str, target: TargetFormat) -> ConvertJob {
    ConvertJob {
        input: PathBuf::from("/Users/ala/Muzyka/film.mp4"),
        input_format: input_format.into(),
        target,
        start_ms: Some(1500),
        duration_ms: Some(2000),
        max_side: Some(640),
        max_output_bytes: 1 << 20,
    }
}

fn output_of(spec: &ExecSpec) -> PathBuf {
    PathBuf::from(
        spec.process
            .args
            .last()
            .unwrap()
            .trim_start_matches("file:"),
    )
}

struct Env {
    dir: PathBuf,
    exec: Arc<FakeExec>,
    t: FfmpegTranscoder,
}

fn env(name: &str) -> Env {
    let dir = std::env::temp_dir().join(format!("alfa-ffmpeg-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    let ffmpeg = dir.join("bin").join("ffmpeg.exe");
    std::fs::write(&ffmpeg, b"atrapa").unwrap();
    let exec = Arc::new(FakeExec::new());
    let t = FfmpegTranscoder::new(exec.clone(), FfmpegConfig::new(ffmpeg, dir.join("work")));
    Env { dir, exec, t }
}

fn writes(bytes: &'static [u8]) -> FakeRun {
    FakeRun::ok("").with_effect(move |spec: &ExecSpec| {
        std::fs::write(output_of(spec), bytes).unwrap();
    })
}

#[test]
fn args_are_closed_and_file_only() {
    let args = ffmpeg_args(&job("mp4", TargetFormat::Mp3), Path::new("/w/out.mp3")).unwrap();
    let joined = args.join(" ");
    for needle in [
        "-nostdin",
        "-n",
        "-protocol_whitelist file",
        "-f mov",
        "-ss 1.500",
        "-t 2.000",
        "-map_metadata -1",
        "-vn",
        "libmp3lame",
    ] {
        assert!(joined.contains(needle), "{needle}: {joined}");
    }
    let i = args.iter().position(|a| a == "-i").unwrap();
    assert_eq!(args[i + 1], "file:/Users/ala/Muzyka/film.mp4");
    assert_eq!(args.last().unwrap(), "file:/w/out.mp3");
    let video = ffmpeg_args(&job("matroska", TargetFormat::Mp4), Path::new("/w/o.mp4"))
        .unwrap()
        .join(" ");
    assert!(
        video.contains("force_divisible_by=2") && video.contains("-f matroska"),
        "{video}"
    );
    let gif = ffmpeg_args(&job("mp4", TargetFormat::Gif), Path::new("/w/o.gif"))
        .unwrap()
        .join(" ");
    assert!(
        gif.contains("fps=10,scale=") && gif.contains("-an"),
        "{gif}"
    );
    for bad in ["heif", "hls", "concat", "tekst"] {
        assert!(matches!(
            ffmpeg_args(&job(bad, TargetFormat::Wav), Path::new("/w/o.wav")),
            Err(ConvertError::Unsupported(_))
        ));
    }
}

#[test]
fn availability_follows_the_sidecar_file() {
    let e = env("avail");
    assert_eq!(e.t.available(), Ok(()));
    let missing = FfmpegTranscoder::new(
        e.exec.clone(),
        FfmpegConfig::new(e.dir.join("brak.exe"), e.dir.join("work")),
    );
    assert!(matches!(
        missing.available(),
        Err(ConvertError::NotInstalled(_))
    ));
    assert!(
        missing
            .convert(
                &job("mp4", TargetFormat::Wav),
                Arc::new(AtomicBool::new(false))
            )
            .is_err()
    );
    assert!(e.exec.runs().is_empty());
    std::fs::remove_dir_all(&e.dir).unwrap();
}

#[test]
fn success_reads_result_and_cleans_up() {
    let e = env("ok");
    e.exec.push(writes(b"ID3wynik"));
    let bytes =
        e.t.convert(
            &job("mp4", TargetFormat::Mp3),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
    assert_eq!(bytes, b"ID3wynik");
    let spec = &e.exec.runs()[0];
    assert!(!output_of(spec).exists(), "plik tymczasowy usunięty");
    assert!(output_of(spec).starts_with(e.dir.join("work")));
    assert_eq!(spec.process.memory_limit_mb, Some(2048));
    assert_eq!(spec.process.cwd, e.dir.join("work"));
    assert!(
        spec.env
            .iter()
            .all(|(k, _)| ["TEMP", "TMP", "SystemRoot"].contains(&k.as_str())),
        "{:?}",
        spec.env
    );
    std::fs::remove_dir_all(&e.dir).unwrap();
}

#[test]
fn failures_timeouts_and_limits() {
    let e = env("fail");
    e.exec.push(FakeRun::exit(
        1,
        "",
        "Invalid data found when processing input",
    ));
    let err =
        e.t.convert(
            &job("mp4", TargetFormat::Wav),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap_err();
    assert!(
        matches!(&err, ConvertError::Failed(m) if m.contains("Invalid data")),
        "{err:?}"
    );
    e.exec.push(FakeRun::ok("").taking(FFMPEG_TIMEOUT_PLUS));
    assert_eq!(
        e.t.convert(
            &job("mp4", TargetFormat::Wav),
            Arc::new(AtomicBool::new(false))
        ),
        Err(ConvertError::Timeout)
    );
    e.exec.push(writes(&[0u8; 4096]));
    let mut small = job("mp4", TargetFormat::Wav);
    small.max_output_bytes = 100;
    assert_eq!(
        e.t.convert(&small, Arc::new(AtomicBool::new(false))),
        Err(ConvertError::TooLarge(100))
    );
    let left: Vec<_> = std::fs::read_dir(e.dir.join("work")).unwrap().collect();
    assert!(left.is_empty(), "bez plików tymczasowych: {left:?}");
    std::fs::remove_dir_all(&e.dir).unwrap();
}

const FFMPEG_TIMEOUT_PLUS: u64 = tools_media_impl::FFMPEG_TIMEOUT_MS + 1;

#[test]
fn cancellation_kills_the_process() {
    let e = env("cancel");
    e.exec.push(FakeRun::hanging());
    let flag = Arc::new(AtomicBool::new(false));
    let setter = flag.clone();
    let t = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        setter.store(true, Ordering::SeqCst);
    });
    assert_eq!(
        e.t.convert(&job("mp4", TargetFormat::Wav), flag),
        Err(ConvertError::Cancelled)
    );
    t.join().unwrap();
    std::fs::remove_dir_all(&e.dir).unwrap();
}
