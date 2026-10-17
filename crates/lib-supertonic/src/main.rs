//! `alfa-supertonic` — proces mowy Supertonic 3.
//!
//! Uruchomienie: `alfa-supertonic --onnx-dir <kat> --voices-dir <kat> --ort <onnxruntime.dll>
//! [--threads 4] [--steps 6] [--speed 1.05] [--lang pl] [--dml <numer karty DXGI>]`. Z `--dml` sieci liczą
//! na karcie przez DirectML (biblioteka ONNX Runtime w wersji DirectML); gdy się nie da, proces mówi to na
//! stderr i liczy na procesorze. Wejście i wyjście: linie JSON (patrz
//! `protocol.rs`). Komunikaty diagnostyczne idą na stderr, żeby nie mieszać się z protokołem.

use std::path::PathBuf;
use std::process::ExitCode;

use lib_supertonic::engine::{Accel, Supertonic};
use lib_supertonic::protocol::{Defaults, serve};

struct Args {
    onnx_dir: PathBuf,
    voices_dir: PathBuf,
    ort: Option<PathBuf>,
    threads: usize,
    accel: Accel,
    defaults: Defaults,
}

fn parse(mut argv: impl Iterator<Item = String>) -> Result<Args, String> {
    let (mut onnx_dir, mut voices_dir, mut ort) = (None, None, None);
    let mut threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(4));
    let mut defaults = Defaults::default();
    let mut accel = Accel::Cpu;
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
            "--dml" => {
                let device = value()?.parse().map_err(|_| "--dml: numer karty")?;
                accel = Accel::DirectMl { device };
            }
            other => return Err(format!("nieznany argument: {other}")),
        }
    }
    Ok(Args {
        onnx_dir: onnx_dir.ok_or("wymagany --onnx-dir")?,
        voices_dir: voices_dir.ok_or("wymagany --voices-dir")?,
        ort,
        threads,
        accel,
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
    let mut engine = load(&args)?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    serve(stdin.lock(), &mut engine, &args.defaults, &mut stdout).map_err(|e| e.to_string())
}

/// Silnik na wybranym urządzeniu; nieudany start na karcie — procesor (z komunikatem na stderr).
fn load(args: &Args) -> Result<Supertonic, String> {
    let on = |accel| Supertonic::load_on(&args.onnx_dir, &args.voices_dir, args.threads, accel);
    match args.accel {
        Accel::Cpu => on(Accel::Cpu).map_err(|e| e.to_string()),
        Accel::DirectMl { device } => match on(args.accel) {
            Ok(engine) => {
                eprintln!("alfa-supertonic: liczę na karcie {device} (DirectML)");
                Ok(engine)
            }
            Err(e) => {
                eprintln!("alfa-supertonic: DirectML niedostępny ({e}) — liczę na procesorze");
                on(Accel::Cpu).map_err(|e| e.to_string())
            }
        },
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Args, String> {
        parse(list.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn dml_flag_selects_the_card_and_defaults_to_cpu() {
        let base = ["--onnx-dir", "m", "--voices-dir", "v"];
        assert_eq!(args(&base).map(|a| a.accel), Ok(Accel::Cpu));
        let with = [&base[..], &["--dml", "1"]].concat();
        assert_eq!(
            args(&with).map(|a| a.accel),
            Ok(Accel::DirectMl { device: 1 })
        );
        let bad = [&base[..], &["--dml", "karta"]].concat();
        assert!(args(&bad).is_err());
        let missing = [&base[..], &["--dml"]].concat();
        assert!(args(&missing).is_err());
    }
}
