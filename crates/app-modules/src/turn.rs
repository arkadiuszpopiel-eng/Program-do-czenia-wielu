//! Koniec wypowiedzi: proces `alfa-smart-turn` z plików aplikacji oraz wspólne miejsce biblioteki
//! ONNX Runtime dla procesów pobocznych mowy.

use std::path::{Path, PathBuf};

use app_api::AppPaths;
use voice_turn_impl::{MODEL_FILE, SidecarCfg};

/// Nazwa pliku biblioteki ONNX Runtime w tym systemie.
pub fn runtime_file() -> &'static str {
    if cfg!(windows) {
        "onnxruntime.dll"
    } else {
        "libonnxruntime.so"
    }
}

/// Biblioteka ONNX Runtime dla procesu `program`: najpierw plik obok programu (wydanie z dołączoną
/// biblioteką), potem pobrana z katalogu (`sidecars/onnxruntime/`). `None` — brak biblioteki.
pub fn onnxruntime(paths: &AppPaths, program: &Path) -> Option<PathBuf> {
    let beside = program.with_file_name(runtime_file());
    if beside.is_file() {
        return Some(beside);
    }
    let shared = paths.sidecars().join("onnxruntime").join(runtime_file());
    shared.is_file().then_some(shared)
}

/// Ustawienia procesu Smart Turn, jeśli są: program, model i biblioteka. `None` — koniec
/// wypowiedzi ocenia model tekstowy.
pub fn smart_turn(paths: &AppPaths) -> Option<SidecarCfg> {
    let program = paths.sidecar("smart-turn", "alfa-smart-turn");
    let model = paths.models().join("turn").join(MODEL_FILE);
    if !program.is_file() || !model.is_file() {
        return None;
    }
    let runtime = onnxruntime(paths, &program)?;
    Some(SidecarCfg::new(program, model, runtime))
}
