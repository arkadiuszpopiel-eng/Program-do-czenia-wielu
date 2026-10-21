//! `alfa-smart-turn` — proces oceny końca wypowiedzi.
//!
//! Uruchomienie: `alfa-smart-turn --model <smart-turn-v3.2-cpu.onnx> --ort <onnxruntime.dll>
//! [--threads 4]`. Wejście i wyjście: linie JSON (patrz `protocol.rs`). Komunikaty diagnostyczne
//! idą na stderr, żeby nie mieszać się z protokołem.

use std::path::PathBuf;
use std::process::ExitCode;

use lib_smart_turn::engine::SmartTurn;
use lib_smart_turn::protocol::serve;

struct Args {
    model: PathBuf,
    ort: Option<PathBuf>,
    threads: usize,
}

fn parse(mut argv: impl Iterator<Item = String>) -> Result<Args, String> {
    let (mut model, mut ort) = (None, None);
    let mut threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(4));
    while let Some(flag) = argv.next() {
        let mut value = || {
            argv.next()
                .ok_or_else(|| format!("brak wartości dla {flag}"))
        };
        match flag.as_str() {
            "--model" => model = Some(PathBuf::from(value()?)),
            "--ort" => ort = Some(PathBuf::from(value()?)),
            "--threads" => threads = value()?.parse().map_err(|_| "--threads: liczba")?,
            other => return Err(format!("nieznany argument: {other}")),
        }
    }
    Ok(Args {
        model: model.ok_or("wymagany --model")?,
        ort,
        threads,
    })
}

fn run() -> Result<(), String> {
    let args = parse(std::env::args().skip(1))?;
    // Telemetria ONNX Runtime wyłączana zaraz po utworzeniu środowiska, także bez `--ort`: oficjalne
    // buildy Microsoftu na Windows wysyłają zdarzenia, a `ort` domyślnie je włącza
    // (`EnableTelemetryEvents`). Zdarzenie startowe z `CreateEnv` i tak wychodzi — pełne wyłączenie
    // wymaga biblioteki bez telemetrii (licencje u źródła, C5.1, 2026-10-21).
    let (builder, from) = match &args.ort {
        Some(dll) => (
            ort::init_from(dll.display().to_string()),
            dll.display().to_string(),
        ),
        None => (ort::init(), "domyślna biblioteka".to_owned()),
    };
    builder
        .with_telemetry(false)
        .commit()
        .map_err(|e| format!("ONNX Runtime ({from}): {e}"))?;
    let mut engine = SmartTurn::load(&args.model, args.threads).map_err(|e| e.to_string())?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    serve(stdin.lock(), &mut engine, &mut stdout).map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("alfa-smart-turn: {message}");
            ExitCode::FAILURE
        }
    }
}
