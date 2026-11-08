//! [`FfmpegTranscoder`] — sidecar `ffmpeg` (instalacja ręczna, pozycja katalogu modeli „do
//! potwierdzenia”) uruchamiany przez `ExecPort` w Job Object z limitem pamięci i czasu, bez
//! dziedziczonego środowiska. Argumenty składane wyłącznie z listy zamkniętej ([`ffmpeg_args`]):
//! wymuszony demuxer wejścia (żadnych list odtwarzania ani wzorców), tylko protokół `file`,
//! metadane usunięte, wynik do pliku tymczasowego w katalogu roboczym (`-n`: nigdy nadpisanie),
//! który narzędzie zapisuje jako nowy plik przez dziennik cofania i usuwa.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use platform_contract::{ExecControl, ExecPort, ExecSpec, ExecTermination, Integrity, ProcessSpec};
use tools_media_contract::{ConvertError, ConvertJob, TargetFormat, Transcoder, input_demuxer};

/// Domyślny limit czasu konwersji (10 min).
pub const FFMPEG_TIMEOUT_MS: u64 = 10 * 60 * 1000;

/// Konfiguracja sidecara.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegConfig {
    /// Plik `ffmpeg(.exe)` (np. `%LOCALAPPDATA%\Alfa\sidecars\ffmpeg\ffmpeg.exe`).
    pub ffmpeg: PathBuf,
    /// Katalog roboczy na wyniki tymczasowe (wewnątrz danych Alfy).
    pub work_dir: PathBuf,
    /// Limit czasu (ms).
    pub timeout_ms: u64,
    /// Limit pamięci drzewa procesów (MB).
    pub memory_limit_mb: u32,
}

impl FfmpegConfig {
    /// Sidecar `ffmpeg` w katalogu `sidecars/ffmpeg` i katalog roboczy.
    pub fn new(ffmpeg: PathBuf, work_dir: PathBuf) -> Self {
        Self {
            ffmpeg,
            work_dir,
            timeout_ms: FFMPEG_TIMEOUT_MS,
            memory_limit_mb: 2048,
        }
    }
}

fn secs(ms: u64) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}

fn scale(max_side: Option<u32>, even: bool) -> Option<String> {
    max_side.map(|n| {
        let even = if even { ":force_divisible_by=2" } else { "" };
        format!("scale=w='min(iw,{n})':h='min(ih,{n})':force_original_aspect_ratio=decrease{even}")
    })
}

fn target_args(job: &ConvertJob) -> Vec<String> {
    let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
    let mut out = match job.target {
        TargetFormat::Wav => s(&["-vn", "-c:a", "pcm_s16le", "-f", "wav"]),
        TargetFormat::Mp3 => s(&["-vn", "-c:a", "libmp3lame", "-q:a", "2", "-f", "mp3"]),
        TargetFormat::Flac => s(&["-vn", "-c:a", "flac", "-f", "flac"]),
        TargetFormat::Ogg => s(&["-vn", "-c:a", "libvorbis", "-q:a", "5", "-f", "ogg"]),
        TargetFormat::Opus => s(&["-vn", "-c:a", "libopus", "-b:a", "128k", "-f", "opus"]),
        TargetFormat::M4a => s(&["-vn", "-c:a", "aac", "-b:a", "192k", "-f", "ipod"]),
        TargetFormat::Mp4 => s(&[
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-crf",
            "23",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-b:a",
            "160k",
            "-movflags",
            "+faststart",
            "-f",
            "mp4",
        ]),
        TargetFormat::Webm => s(&[
            "-c:v",
            "libvpx-vp9",
            "-crf",
            "32",
            "-b:v",
            "0",
            "-c:a",
            "libopus",
            "-b:a",
            "128k",
            "-f",
            "webm",
        ]),
        TargetFormat::Gif => s(&["-an", "-loop", "0", "-f", "gif"]),
        TargetFormat::Png => s(&["-an", "-frames:v", "1", "-update", "1", "-f", "image2"]),
        TargetFormat::Jpg => s(&[
            "-an",
            "-frames:v",
            "1",
            "-q:v",
            "2",
            "-update",
            "1",
            "-f",
            "image2",
        ]),
    };
    let filter = match job.target {
        TargetFormat::Gif => Some(match scale(job.max_side, false) {
            Some(sc) => format!("fps=10,{sc}"),
            None => "fps=10".into(),
        }),
        TargetFormat::Mp4 | TargetFormat::Webm => scale(job.max_side, true),
        TargetFormat::Png | TargetFormat::Jpg => scale(job.max_side, false),
        _ => None,
    };
    if let Some(vf) = filter {
        out.splice(0..0, ["-vf".to_owned(), vf]);
    }
    out
}

/// Argumenty ffmpeg dla zadania (lista zamknięta; ścieżki jako `file:` — bez innych protokołów).
pub fn ffmpeg_args(job: &ConvertJob, output: &Path) -> Result<Vec<String>, ConvertError> {
    let demuxer = input_demuxer(&job.input_format).ok_or_else(|| {
        ConvertError::Unsupported(format!("wejście w formacie {}", job.input_format))
    })?;
    let mut args: Vec<String> = [
        "-nostdin",
        "-hide_banner",
        "-nostats",
        "-loglevel",
        "error",
        "-n",
    ]
    .iter()
    .map(|a| (*a).to_owned())
    .collect();
    if let Some(start) = job.start_ms {
        args.extend(["-ss".to_owned(), secs(start)]);
    }
    args.extend([
        "-protocol_whitelist".to_owned(),
        "file".to_owned(),
        "-f".to_owned(),
        demuxer.to_owned(),
        "-i".to_owned(),
        format!("file:{}", job.input.display()),
    ]);
    if let Some(d) = job.duration_ms {
        args.extend(["-t".to_owned(), secs(d)]);
    }
    args.extend(["-map_metadata".to_owned(), "-1".to_owned()]);
    args.extend(target_args(job));
    args.push(format!("file:{}", output.display()));
    Ok(args)
}

/// Konwerter na sidecarze `ffmpeg`.
#[derive(Clone)]
pub struct FfmpegTranscoder {
    exec: Arc<dyn ExecPort>,
    config: FfmpegConfig,
}

impl std::fmt::Debug for FfmpegTranscoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FfmpegTranscoder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl FfmpegTranscoder {
    /// Konwerter nad portem uruchamiania (ten sam rejestr Job Objects co kill-switch).
    pub fn new(exec: Arc<dyn ExecPort>, config: FfmpegConfig) -> Self {
        Self { exec, config }
    }

    fn env(&self) -> Vec<(String, String)> {
        let work = self.config.work_dir.display().to_string();
        let mut env = vec![("TEMP".to_owned(), work.clone()), ("TMP".to_owned(), work)];
        if let Ok(root) = std::env::var("SystemRoot") {
            env.push(("SystemRoot".to_owned(), root));
        }
        env
    }

    fn run(&self, spec: ExecSpec, cancel: &AtomicBool) -> Result<ExecTermination, ConvertError> {
        let control = ExecControl::new();
        let done = AtomicBool::new(false);
        let output = std::thread::scope(|s| {
            s.spawn(|| {
                while !done.load(Ordering::SeqCst) {
                    if cancel.load(Ordering::SeqCst) {
                        control.cancel();
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            });
            let r = self.exec.run_captured(spec, &control);
            done.store(true, Ordering::SeqCst);
            r
        })
        .map_err(|e| ConvertError::Failed(format!("uruchomienie ffmpeg: {e}")))?;
        match output.termination {
            ExecTermination::Exited(0) => Ok(ExecTermination::Exited(0)),
            ExecTermination::Exited(code) => {
                let err = String::from_utf8_lossy(&output.stderr);
                let tail: String = err.lines().rev().take(3).collect::<Vec<_>>().join(" | ");
                Err(ConvertError::Failed(format!(
                    "ffmpeg zakończył się kodem {code}: {tail}"
                )))
            }
            ExecTermination::TimedOut => Err(ConvertError::Timeout),
            ExecTermination::Cancelled | ExecTermination::Killed => Err(ConvertError::Cancelled),
        }
    }
}

impl Transcoder for FfmpegTranscoder {
    fn available(&self) -> Result<(), ConvertError> {
        if self.config.ffmpeg.is_file() {
            Ok(())
        } else {
            Err(ConvertError::NotInstalled(format!(
                "brak {}",
                self.config.ffmpeg.display()
            )))
        }
    }

    fn convert(&self, job: &ConvertJob, cancel: Arc<AtomicBool>) -> Result<Vec<u8>, ConvertError> {
        self.available()?;
        let work = &self.config.work_dir;
        std::fs::create_dir_all(work)
            .map_err(|e| ConvertError::Failed(format!("katalog roboczy: {e}")))?;
        let output = work.join(format!("{}.{}", uuid::Uuid::new_v4(), job.target.ext()));
        let spec = ExecSpec {
            process: ProcessSpec {
                cmd: self.config.ffmpeg.clone(),
                args: ffmpeg_args(job, &output)?,
                cwd: work.clone(),
                integrity: Integrity::Medium,
                memory_limit_mb: Some(self.config.memory_limit_mb),
            },
            raw_args: None,
            env: self.env(),
            timeout_ms: self.config.timeout_ms,
            max_output_bytes: 64 * 1024,
        };
        let result = self.run(spec, &cancel).and_then(|_| {
            let len = std::fs::metadata(&output)
                .map_err(|e| ConvertError::Failed(format!("brak wyniku: {e}")))?
                .len();
            if len > job.max_output_bytes {
                return Err(ConvertError::TooLarge(job.max_output_bytes));
            }
            std::fs::read(&output).map_err(|e| ConvertError::Failed(format!("odczyt wyniku: {e}")))
        });
        if output.exists()
            && let Err(e) = std::fs::remove_file(&output)
        {
            tracing::warn!(error = %e, "nie usunięto pliku tymczasowego konwersji");
        }
        result
    }
}
