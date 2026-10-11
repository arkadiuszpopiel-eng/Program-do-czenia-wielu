//! `alfa-supertonic` — proces mowy Supertonic 3.
//!
//! Uruchomienie: `alfa-supertonic --onnx-dir <kat> --voices-dir <kat> --ort <onnxruntime.dll>
//! [--threads 4] [--steps 6] [--speed 1.05] [--lang pl]`. Wejście i wyjście: linie JSON (patrz
//! `protocol.rs`). Komunikaty diagnostyczne idą na stderr, żeby nie mieszać się z protokołem.

use std::path::PathBuf;
use std::process::ExitCode;

use lib_supertonic::engine::Supertonic;
use lib_supertonic::protocol::{Defaults, serve};

struct Args {
    onnx_dir: PathBuf,
    voices_dir: PathBuf,
    ort: Option<PathBuf>,
    threads: usize,
    defaults: Defaults,
}

fn parse(mut argv: impl Iterator<Item = String>) -> Result<Args, String> {
    let (mut onnx_dir, mut voices_dir, mut ort) = (None, None, None);
    let mut threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(4));
    let mut defaults = Defaults::default();
    while let Some(flag) = argv.next() {
        let mut value = || {
            argv.next()
                .ok_or_else(|| format!("brak wartości dla {flag}"))
        };
        match flag.as_str() {
            "--onnx-dir" => onnx_dir = Some(PathBuf::from(value()?)),
            "--voices-dir" => voices_dir = Some(PathBuf::from(value()?)),
            "--ort" => ort = Some(PathBuf::from(value()?)),
            "--threads" => threads = value()?.parse().map_err(|_| "--threads: liczba")?,
            "--steps" => defaults.steps = value()?.parse().map_err(|_| "--steps: liczba")?,
            "--speed" => defaults.speed = value()?.parse().map_err(|_| "--speed: liczba")?,
            "--lang" => defaults.lang = value()?,
            other => return Err(format!("nieznany argument: {other}")),
        }
    }
    Ok(Args {
        onnx_dir: onnx_dir.ok_or("wymagany --onnx-dir")?,
        voices_dir: voices_dir.ok_or("wymagany --voices-dir")?,
        ort,
        threads,
        defaults,
    })
}

fn run() -> Result<(), String> {
    let args = parse(std::env::args().skip(1))?;
    if let Some(dll) = &args.ort {
        ort::init_from(dll.display().to_string())
            .commit()
            .map_err(|e| format!("ONNX Runtime ({}): {e}", dll.display()))?;
    }
    let mut engine = Supertonic::load(&args.onnx_dir, &args.voices_dir, args.threads)
        .map_err(|e| e.to_string())?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    serve(stdin.lock(), &mut engine, &args.defaults, &mut stdout).map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("alfa-supertonic: {message}");
            ExitCode::FAILURE
        }
    }
}
